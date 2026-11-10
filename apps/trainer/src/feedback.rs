//! What the player hears and sees the moment a shot lands: sounds, hit markers, streaks and why a hit
//! did not count. The logic here is pure; the game systems in main.rs only display it.

use crate::drill_rules::Rejection;
use std::sync::OnceLock;

const SAMPLE_RATE: u32 = 44_100;

/// A short tone as a complete in-memory WAV file (16-bit mono PCM).
pub fn tone_wav(freq_hz: f32, ms: u32, volume: f32) -> Vec<u8> {
    let samples = SAMPLE_RATE * ms / 1000;
    let data_len = samples * 2;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    let amplitude = f32::from(i16::MAX) * volume.clamp(0.0, 1.0);
    for i in 0..samples {
        let t = i as f32 / SAMPLE_RATE as f32;
        let progress = i as f32 / samples as f32;
        // Fast attack and exponential decay keep it a click, not a beep.
        let envelope = (progress * 40.0).min(1.0) * (-progress * 5.0).exp();
        let value = (t * freq_hz * std::f32::consts::TAU).sin() * envelope * amplitude;
        wav.extend_from_slice(&(value as i16).to_le_bytes());
    }
    wav
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Hit,
    Miss,
}

static VOLUME: OnceLock<f32> = OnceLock::new();
static HIT: OnceLock<Vec<u8>> = OnceLock::new();
static MISS: OnceLock<Vec<u8>> = OnceLock::new();

/// Sets the volume once at startup (0 turns sound off).
pub fn set_volume(percent: u32) {
    let _ = VOLUME.set(percent.min(100) as f32 / 100.0);
}

pub fn play(sound: Sound) {
    let volume = VOLUME.get().copied().unwrap_or(0.6);
    if volume <= 0.0 {
        return;
    }
    let bytes = match sound {
        Sound::Hit => HIT.get_or_init(|| tone_wav(1320.0, 70, volume)),
        Sound::Miss => MISS.get_or_init(|| tone_wav(240.0, 110, volume * 0.8)),
    };
    play_bytes(bytes);
}

#[cfg(windows)]
fn play_bytes(bytes: &'static [u8]) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    // SAFETY: the buffer is a complete WAV file held in a static, so it outlives the asynchronous play.
    unsafe {
        PlaySoundW(
            bytes.as_ptr() as *const u16,
            std::ptr::null_mut(),
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

#[cfg(not(windows))]
fn play_bytes(_bytes: &'static [u8]) {}

/// What a single shot meant, for the marker and the message under the crosshair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotOutcome {
    Hit,
    Miss,
    Rejected(Rejection),
}

/// Running state of the feedback shown during a session.
#[derive(Debug, Clone, Default)]
pub struct FeedbackState {
    pub streak: u32,
    pub best_streak: u32,
    seen_hits: u32,
    seen_misses: u32,
    pub pending_rejection: Option<Rejection>,
}

impl FeedbackState {
    /// Compares the session counters with the last frame and reports what happened.
    pub fn observe(&mut self, hits: u32, misses: u32) -> Option<ShotOutcome> {
        let outcome = if hits > self.seen_hits {
            self.streak += hits - self.seen_hits;
            self.best_streak = self.best_streak.max(self.streak);
            Some(ShotOutcome::Hit)
        } else if misses > self.seen_misses {
            self.streak = 0;
            Some(
                self.pending_rejection
                    .take()
                    .map_or(ShotOutcome::Miss, ShotOutcome::Rejected),
            )
        } else {
            None
        };
        self.seen_hits = hits;
        self.seen_misses = misses;
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_matches_its_length() {
        let wav = tone_wav(1000.0, 50, 0.5);
        assert_eq!(&wav[0..4], b"RIFF");
        let riff = u32::from_le_bytes(wav[4..8].try_into().unwrap());
        assert_eq!(riff as usize, wav.len() - 8);
        let data = u32::from_le_bytes(wav[40..44].try_into().unwrap());
        assert_eq!(data as usize, wav.len() - 44);
        assert_eq!(data, 44_100 * 50 / 1000 * 2);
    }

    #[test]
    fn silent_volume_is_silent() {
        let wav = tone_wav(1000.0, 20, 0.0);
        assert!(wav[44..].iter().all(|b| *b == 0));
    }

    #[test]
    fn streak_counts_hits_and_resets_on_a_miss() {
        let mut f = FeedbackState::default();
        assert_eq!(f.observe(1, 0), Some(ShotOutcome::Hit));
        assert_eq!(f.observe(2, 0), Some(ShotOutcome::Hit));
        assert_eq!(f.observe(2, 0), None);
        assert_eq!(f.streak, 2);
        assert_eq!(f.observe(2, 1), Some(ShotOutcome::Miss));
        assert_eq!(f.streak, 0);
        assert_eq!(f.best_streak, 2);
    }

    #[test]
    fn a_rejected_hit_explains_itself() {
        let mut f = FeedbackState {
            pending_rejection: Some(Rejection::Overshot),
            ..Default::default()
        };
        assert_eq!(
            f.observe(0, 1),
            Some(ShotOutcome::Rejected(Rejection::Overshot))
        );
        assert_eq!(f.observe(0, 2), Some(ShotOutcome::Miss));
    }
}
