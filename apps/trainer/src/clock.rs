//! Dedicated QPC session clock providing uniform microsecond timestamps
//! across all trainer subsystems (input, targets, frames, shots).

use windows_sys::Win32::System::Performance::QueryPerformanceCounter;

#[derive(Clone, Copy)]
pub struct SessionClock {
    pub qpc_start: i64,
    pub qpc_freq: i64,
}

impl SessionClock {
    pub fn new(qpc_start: i64, qpc_freq: i64) -> Self {
        Self {
            qpc_start,
            qpc_freq,
        }
    }

    /// Resets the baseline starting QPC counter to the current instant.
    pub fn reset_start(&mut self) {
        let mut qpc_now = 0i64;
        unsafe {
            QueryPerformanceCounter(&mut qpc_now);
        }
        self.qpc_start = qpc_now;
    }

    /// Queries the current QPC counter and computes microseconds elapsed since session start.
    #[inline]
    pub fn now_us(&self) -> u64 {
        let mut qpc_now = 0i64;
        unsafe {
            QueryPerformanceCounter(&mut qpc_now);
        }
        let qpc_diff = (qpc_now - self.qpc_start).max(0) as u128;
        ((qpc_diff * 1_000_000u128) / (self.qpc_freq as u128)) as u64
    }

    /// Converts an elapsed microsecond timestamp to seconds as f64.
    #[inline]
    pub fn us_to_s(us: u64) -> f64 {
        us as f64 / 1_000_000.0
    }
}
