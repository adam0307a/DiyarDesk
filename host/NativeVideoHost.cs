using System;
using System.IO;
using System.Windows.Forms;

// Built-in Windows hardware-accelerated video wallpaper backend using WPF MediaElement
// hosted inside WinForms ElementHost (0 external mpv.exe, 0 WebView2 processes).
sealed class NativeVideoHost : IDisposable
{
    public event Action<string> PlaybackFailed;

    public bool IsRunning { get { return running; } }
    public string CurrentPath { get { return currentPath; } }

    bool running;
    bool paused;
    string currentPath;
    double speed = 1.0;
    double brightness = 1.0;

    public static bool IsAvailable()
    {
#if HAS_WPF_VIDEO
        return true;
#else
        return false;
#endif
    }

#if HAS_WPF_VIDEO
    Control parentCtrl;
    System.Windows.Forms.Integration.ElementHost host;
    System.Windows.Controls.MediaElement media;
    System.Windows.Shapes.Rectangle dimOverlay;
    bool disposed;

    public bool Start(Control parent, string videoPath, double speedVal, double brightnessVal)
    {
        if (disposed || parent == null || parent.IsDisposed) return false;
        if (string.IsNullOrEmpty(videoPath) || !File.Exists(videoPath)) return false;

        if (running && host != null && !host.IsDisposed
            && string.Equals(currentPath, videoPath, StringComparison.OrdinalIgnoreCase))
        {
            UpdateParams(speedVal, brightnessVal);
            return true;
        }

        StopHost();
        parentCtrl = parent;

        try
        {
            var me = new System.Windows.Controls.MediaElement();
            me.LoadedBehavior = System.Windows.Controls.MediaState.Manual;
            me.UnloadedBehavior = System.Windows.Controls.MediaState.Manual;
            me.Stretch = System.Windows.Media.Stretch.UniformToFill;
            me.IsMuted = true;
            me.Volume = 0.0;
            me.ScrubbingEnabled = false;
            me.HorizontalAlignment = System.Windows.HorizontalAlignment.Stretch;
            me.VerticalAlignment = System.Windows.VerticalAlignment.Stretch;

            var overlay = new System.Windows.Shapes.Rectangle();
            overlay.Fill = System.Windows.Media.Brushes.Black;
            overlay.IsHitTestVisible = false;
            overlay.Opacity = 0.0;

            var grid = new System.Windows.Controls.Grid();
            grid.Background = System.Windows.Media.Brushes.Black;
            grid.Children.Add(me);
            grid.Children.Add(overlay);

            var eh = new System.Windows.Forms.Integration.ElementHost();
            eh.Dock = DockStyle.Fill;
            eh.BackColor = System.Drawing.Color.Black;
            eh.TabStop = false;
            eh.Child = grid;

            me.MediaEnded += delegate
            {
                if (disposed || media != me) return;
                try
                {
                    me.Position = TimeSpan.Zero;
                    if (!paused) me.Play();
                }
                catch { }
            };

            me.MediaFailed += delegate(object sender, System.Windows.ExceptionRoutedEventArgs e)
            {
                string err = (e != null && e.ErrorException != null) ? e.ErrorException.Message : "media failed";
                Log.Write("Yerel WPF video hatasi (WebView2 yedegine donuluyor): " + err);
                StopHost();
                Action<string> cb = PlaybackFailed;
                if (cb != null) cb(err);
            };

            media = me;
            dimOverlay = overlay;
            host = eh;

            parent.Controls.Add(eh);
            eh.BringToFront();
            eh.CreateControl();

            me.Source = new Uri(videoPath, UriKind.Absolute);
            currentPath = videoPath;
            running = true;
            paused = false;
            UpdateParams(speedVal, brightnessVal);
            me.Play();
            return true;
        }
        catch (Exception ex)
        {
            Log.Write("Yerel WPF video baslatilamadi: " + ex.Message);
            StopHost();
            return false;
        }
    }

    public void UpdateParams(double speedVal, double brightnessVal)
    {
        speed = Math.Max(0.25, Math.Min(2.0, speedVal));
        brightness = Math.Max(0.4, Math.Min(1.4, brightnessVal));
        if (media != null)
        {
            try { media.SpeedRatio = speed; } catch { }
        }
        if (dimOverlay != null)
        {
            try
            {
                if (brightness < 0.99)
                {
                    dimOverlay.Fill = System.Windows.Media.Brushes.Black;
                    dimOverlay.Opacity = Math.Min(0.75, Math.Max(0.0, 1.0 - brightness));
                }
                else if (brightness > 1.01)
                {
                    dimOverlay.Fill = System.Windows.Media.Brushes.White;
                    dimOverlay.Opacity = Math.Min(0.22, (brightness - 1.0) * 0.45);
                }
                else
                {
                    dimOverlay.Opacity = 0.0;
                }
            }
            catch { }
        }
    }

    public void SetPaused(bool pause)
    {
        if (!running || media == null || paused == pause) return;
        paused = pause;
        try
        {
            if (pause) media.Pause();
            else media.Play();
        }
        catch { }
    }

    void StopHost()
    {
        running = false;
        paused = false;
        currentPath = null;
        if (media != null)
        {
            try { media.Stop(); } catch { }
            try { media.Source = null; } catch { }
            try { media.Close(); } catch { }
            media = null;
        }
        dimOverlay = null;
        if (host != null)
        {
            try { host.Visible = false; } catch { }
            try { host.Child = null; } catch { }
            try
            {
                if (parentCtrl != null && !parentCtrl.IsDisposed)
                    parentCtrl.Controls.Remove(host);
            }
            catch { }
            try { host.Dispose(); } catch { }
            host = null;
        }
    }

    public void Dispose()
    {
        disposed = true;
        StopHost();
    }
#else
    public bool Start(Control parent, string videoPath, double speedVal, double brightnessVal)
    {
        return false;
    }

    public void UpdateParams(double speedVal, double brightnessVal) { }
    public void SetPaused(bool pause) { }
    public void Dispose() { running = false; currentPath = null; }
#endif
}
