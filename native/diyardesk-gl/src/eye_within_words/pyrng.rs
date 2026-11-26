//! CPython-compatible Mersenne Twister (MT19937) with the exact `random`,
//! `uniform`, `randint`, `choice` and `getrandbits` semantics used by
//! `eye_python/main.py`, so a given seed reproduces the Python layout
//! decisions 1:1.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

pub struct PyRng {
    mt: [u32; N],
    mti: usize,
}

impl PyRng {
    /// Equivalent to Python's `random.seed(<int>)` on a fresh `random.Random`.
    pub fn new(seed: u64) -> Self {
        let mut r = Self {
            mt: [0; N],
            mti: N + 1,
        };
        r.seed_int(seed);
        r
    }

    /// CPython `_random.Random.seed(int)`: key = 32-bit little-endian limbs of abs(n).
    fn seed_int(&mut self, seed: u64) {
        let mut key: Vec<u32> = Vec::new();
        let mut n = seed;
        if n == 0 {
            key.push(0);
        } else {
            while n > 0 {
                key.push((n & 0xFFFF_FFFF) as u32);
                n >>= 32;
            }
        }
        self.init_by_array(&key);
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            self.mt[i] = 1812433253u32
                .wrapping_mul(self.mt[i - 1] ^ (self.mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        self.mti = N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19650218);
        let mut i = 1usize;
        let mut j = 0usize;
        let mut k = N.max(key.len());
        while k > 0 {
            self.mt[i] = (self.mt[i]
                ^ ((self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_mul(1664525)))
            .wrapping_add(key[j])
            .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = N - 1;
        while k > 0 {
            self.mt[i] = (self.mt[i]
                ^ ((self.mt[i - 1] ^ (self.mt[i - 1] >> 30)).wrapping_mul(1566083941)))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            k -= 1;
        }
        self.mt[0] = 0x8000_0000;
    }

    fn next_u32(&mut self) -> u32 {
        if self.mti >= N {
            for kk in 0..(N - M) {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ if y & 1 != 0 { MATRIX_A } else { 0 };
            }
            for kk in (N - M)..(N - 1) {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk - (N - M)] ^ (y >> 1) ^ if y & 1 != 0 { MATRIX_A } else { 0 };
            }
            let y = (self.mt[N - 1] & UPPER_MASK) | (self.mt[0] & LOWER_MASK);
            self.mt[N - 1] = self.mt[M - 1] ^ (y >> 1) ^ if y & 1 != 0 { MATRIX_A } else { 0 };
            self.mti = 0;
        }
        let mut y = self.mt[self.mti];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// CPython `random.random()` (53-bit).
    pub fn random(&mut self) -> f64 {
        let a = (self.next_u32() >> 5) as f64;
        let b = (self.next_u32() >> 6) as f64;
        (a * 67108864.0 + b) * (1.0 / 9007199254740992.0)
    }

    /// CPython `random.uniform(a, b)`: `a + (b - a) * self.random()`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// CPython `random.getrandbits(k)` for 1 <= k <= 64.
    pub fn getrandbits(&mut self, k: u32) -> u64 {
        assert!(k >= 1 && k <= 64, "getrandbits supports 1..=64 bits");
        if k <= 32 {
            return (self.next_u32() >> (32 - k)) as u64;
        }
        let words = ((k - 1) / 32 + 1) as usize;
        let mut result: u64 = 0;
        for i in 0..words {
            let mut r = self.next_u32() as u64;
            let rem = k - (i as u32) * 32;
            if rem < 32 {
                r >>= 32 - rem;
            }
            result = (result << 32) | r;
        }
        result
    }

    /// CPython `Random._randbelow(n)` (getrandbits rejection sampling).
    fn randbelow(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let k = 64 - n.leading_zeros();
        let mut r = self.getrandbits(k);
        while r >= n {
            r = self.getrandbits(k);
        }
        r
    }

    /// CPython `random.randint(a, b)` inclusive.
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        a + self.randbelow((b - a + 1) as u64) as i64
    }

    /// CPython `random.choice(seq)` (`seq[self._randbelow(len(seq))]`).
    pub fn choice<'a, T>(&mut self, seq: &'a [T]) -> &'a T {
        &seq[self.randbelow(seq.len() as u64) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mt19937_reference_vectors() {
        // Exact CPython `random.Random(seed).getrandbits(32)` sequences.
        let mut r = PyRng::new(5489);
        let expected: [u32; 6] = [
            3382763572, 956215839, 417760592, 166104981, 4181578304, 1596625880,
        ];
        for e in expected {
            assert_eq!(r.getrandbits(32) as u32, e);
        }
        let mut r2 = PyRng::new(12345);
        let expected2: [u32; 4] = [1789368711, 3146859322, 43676229, 3522623596];
        for e in expected2 {
            assert_eq!(r2.getrandbits(32) as u32, e);
        }
    }

    #[test]
    fn uniform_and_randint_ranges() {
        let mut r = PyRng::new(99);
        for _ in 0..200 {
            let u = r.uniform(0.115, 0.140);
            assert!((0.115..=0.140).contains(&u));
            let i = r.randint(200, 238);
            assert!((200..=238).contains(&i));
        }
    }
}
