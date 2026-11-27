/// Lightweight local audio peak & beat onset follower for wallpaper reactivity.
///
/// Receives scalar output peaks (`0.0..=1.0`) from the host's local loopback
/// peak meter and produces a smooth envelope (`level`) plus transient beat
/// onset pulse (`beat`), automatically fading cleanly to `0.0` when paused
/// or silent so idle rendering incurs zero visual artifacts or GPU overhead.
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioReactiveState {
    raw_peak: f32,
    level: f32,
    energy_avg: f32,
    beat_pulse: f32,
    stale_sec: f32,
}

impl AudioReactiveState {
    pub fn push_peak(&mut self, peak: f32) {
        if !peak.is_finite() {
            return;
        }
        let p = peak.clamp(0.0, 1.0);
        self.raw_peak = p;
        self.stale_sec = 0.0;
        let threshold = self.energy_avg * 1.32 + 0.06;
        if p > threshold {
            let delta = ((p - self.energy_avg) * 2.2).clamp(0.0, 1.0);
            self.beat_pulse = self.beat_pulse.max(delta);
        }
    }

    pub fn step(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.25);
        self.stale_sec += dt;
        if self.stale_sec > 0.75 {
            self.raw_peak *= (-6.0 * dt).exp();
            if self.raw_peak < 0.0005 {
                self.raw_peak = 0.0;
            }
        }

        let rate = if self.raw_peak > self.level { 18.0 } else { 6.0 };
        self.level += (self.raw_peak - self.level) * (dt * rate).min(1.0);
        self.energy_avg += (self.raw_peak - self.energy_avg) * (dt * 1.8).min(1.0);
        self.beat_pulse *= (-4.8 * dt).exp();

        if self.level < 0.0005 {
            self.level = 0.0;
        }
        if self.energy_avg < 0.0005 {
            self.energy_avg = 0.0;
        }
        if self.beat_pulse < 0.0005 {
            self.beat_pulse = 0.0;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn beat(&self) -> f32 {
        self.beat_pulse
    }

    pub fn is_active(&self) -> bool {
        self.level > 0.002 || self.beat_pulse > 0.002
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_silent_and_inactive() {
        let mut a = AudioReactiveState::default();
        assert!(!a.is_active());
        assert_eq!(a.level(), 0.0);
        assert_eq!(a.beat(), 0.0);
        a.step(0.033);
        assert!(!a.is_active());
    }

    #[test]
    fn beat_onset_triggers_pulse_and_decays_to_zero_on_silence() {
        let mut a = AudioReactiveState::default();
        // Low ambient bed
        for _ in 0..8 {
            a.push_peak(0.05);
            a.step(0.03);
        }
        // Kick/beat transient
        a.push_peak(0.85);
        assert!(a.beat() > 0.5, "beat pulse should react sharply to onset");
        a.step(0.033);
        assert!(a.is_active());
        assert!(a.level() > 0.1);

        // Stop pushing updates -> stale timeout fades cleanly back to silent
        for _ in 0..90 {
            a.step(0.05);
        }
        assert!(!a.is_active());
        assert_eq!(a.level(), 0.0);
        assert_eq!(a.beat(), 0.0);
    }

    #[test]
    fn sanitizes_nan_and_out_of_range_peaks() {
        let mut a = AudioReactiveState::default();
        a.push_peak(f32::NAN);
        a.push_peak(f32::INFINITY);
        a.step(0.033);
        assert!(!a.is_active());

        a.push_peak(4.2);
        a.step(0.1);
        assert!(a.level() <= 1.0 && a.beat() <= 1.0);
    }
}
