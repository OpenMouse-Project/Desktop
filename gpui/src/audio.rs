//! Audio feedback for game notifications: a short, quiet chime, synthesized in
//! memory (no sound files to ship) and played through each platform's own
//! audio call, so it adds no dependency and never starts a helper process on
//! the two main platforms.
//!
//! - macOS: `NSSound` from the WAV bytes
//! - Windows: `PlaySound` straight from memory
//! - Linux: `paplay`/`aplay` on a cached copy (best effort)

use std::sync::OnceLock;

/// Which chime: `Success` is a soft rising pair, `Error` a soft falling pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chime {
    Success,
    Error,
}

const SAMPLE_RATE: u32 = 44_100;
/// Peak level of the chime, 0..1. Quiet on purpose: it should not startle someone
/// mid-game or fight the game's own audio.
const PEAK: f32 = 0.30;

/// One note: a sine plus a faint octave for a glassy body, with a quick attack
/// and an exponential decay.
fn note(buffer: &mut [f32], start_s: f32, freq: f32, decay_s: f32) {
    let first = (start_s * SAMPLE_RATE as f32) as usize;
    for (i, slot) in buffer.iter_mut().enumerate().skip(first) {
        let t = (i - first) as f32 / SAMPLE_RATE as f32;
        let attack = (t / 0.004).min(1.0);
        let envelope = attack * (-t / decay_s).exp();
        // Stop once the note has died away — but not during the attack, where the
        // envelope starts at zero.
        if t > 0.004 && envelope < 0.0005 {
            break;
        }
        let phase = std::f32::consts::TAU * freq * t;
        *slot += envelope * (phase.sin() + 0.16 * (2.0 * phase).sin());
    }
}

/// 16-bit mono PCM samples for a chime, normalised to [`PEAK`] and faded out at
/// the end so it can't click.
fn samples(chime: Chime) -> Vec<i16> {
    // Long enough that every note has decayed to near-silence before the end.
    let seconds = 0.52;
    let mut buffer = vec![0.0f32; (seconds * SAMPLE_RATE as f32) as usize];
    match chime {
        // G5 then D6: bright and brief.
        Chime::Success => {
            note(&mut buffer, 0.00, 783.99, 0.075);
            note(&mut buffer, 0.085, 1174.66, 0.11);
        }
        // A4 then E4: the same shape, lower and falling.
        Chime::Error => {
            note(&mut buffer, 0.00, 440.00, 0.09);
            note(&mut buffer, 0.10, 329.63, 0.11);
        }
    }
    let peak = buffer.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(f32::EPSILON);
    let gain = PEAK / peak;
    let fade = (0.012 * SAMPLE_RATE as f32) as usize;
    let len = buffer.len();
    buffer
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let tail = len.saturating_sub(i).min(fade) as f32 / fade as f32;
            (s * gain * tail * i16::MAX as f32) as i16
        })
        .collect()
}

/// A complete WAV file (RIFF, 16-bit mono PCM) for the chime.
pub fn wav(chime: Chime) -> Vec<u8> {
    let pcm = samples(chime);
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + pcm.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// Each chime's bytes, built once and kept for the life of the process (Windows
/// plays from memory asynchronously and needs the buffer to stay valid).
fn cached(chime: Chime) -> &'static [u8] {
    static SUCCESS: OnceLock<Vec<u8>> = OnceLock::new();
    static ERROR: OnceLock<Vec<u8>> = OnceLock::new();
    match chime {
        Chime::Success => SUCCESS.get_or_init(|| wav(Chime::Success)),
        Chime::Error => ERROR.get_or_init(|| wav(Chime::Error)),
    }
}

/// Plays the chime without waiting for it to finish. Returns whether the system
/// took it; a failure is never worth surfacing — it's feedback, not function.
pub fn play(chime: Chime) -> bool {
    let bytes = cached(chime);
    #[cfg(target_os = "macos")]
    return macos::play(bytes);
    #[cfg(windows)]
    return windows::play(bytes);
    #[cfg(not(any(target_os = "macos", windows)))]
    return other::play(chime, bytes);
}

#[cfg(target_os = "macos")]
mod macos {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::Mutex;

    /// The sound currently playing. An `NSSound` that is released stops, so it is
    /// kept until the next one replaces it.
    static CURRENT: Mutex<usize> = Mutex::new(0);

    pub fn play(bytes: &[u8]) -> bool {
        unsafe {
            let data: id = msg_send![class!(NSData), dataWithBytes: bytes.as_ptr() length: bytes.len()];
            if data == nil {
                return false;
            }
            let sound: id = msg_send![class!(NSSound), alloc];
            let sound: id = msg_send![sound, initWithData: data];
            if sound == nil {
                return false;
            }
            let started: objc::runtime::BOOL = msg_send![sound, play];
            let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
            if *current != 0 {
                let old = *current as id;
                let _: () = msg_send![old, stop];
                let _: () = msg_send![old, release];
            }
            *current = sound as usize;
            started == objc::runtime::YES
        }
    }
}

#[cfg(windows)]
mod windows {
    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundW(sound: *const u8, module: isize, flags: u32) -> i32;
    }
    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;

    /// `bytes` is `'static`: asynchronous in-memory playback keeps reading it
    /// after this returns.
    pub fn play(bytes: &'static [u8]) -> bool {
        unsafe { PlaySoundW(bytes.as_ptr(), 0, SND_MEMORY | SND_ASYNC | SND_NODEFAULT) != 0 }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod other {
    use super::Chime;

    pub fn play(chime: Chime, bytes: &[u8]) -> bool {
        let Some(dir) = dirs::cache_dir().map(|d| d.join("openmouse")) else { return false };
        let path = dir.join(match chime {
            Chime::Success => "chime-success.wav",
            Chime::Error => "chime-error.wav",
        });
        if !path.is_file() {
            let _ = std::fs::create_dir_all(&dir);
            if std::fs::write(&path, bytes).is_err() {
                return false;
            }
        }
        // PulseAudio/PipeWire first, then ALSA; whichever exists plays it.
        for player in [("paplay", vec![]), ("aplay", vec!["-q"])] {
            let mut command = std::process::Command::new(player.0);
            command
                .args(&player.1)
                .arg(&path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            if crate::proc::quiet(&mut command).spawn().is_ok() {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
    }

    #[test]
    fn writes_a_valid_wav() {
        for chime in [Chime::Success, Chime::Error] {
            let w = wav(chime);
            assert_eq!(&w[0..4], b"RIFF");
            assert_eq!(&w[8..16], b"WAVEfmt ");
            assert_eq!(&w[36..40], b"data");
            let data = u32_at(&w, 40) as usize;
            assert_eq!(w.len(), 44 + data, "data chunk length matches the file");
            assert_eq!(u32_at(&w, 4) as usize, w.len() - 8, "RIFF length matches the file");
            assert_eq!(u32_at(&w, 24), SAMPLE_RATE);
        }
    }

    #[test]
    fn stays_quiet_and_never_clips() {
        for chime in [Chime::Success, Chime::Error] {
            let peak = samples(chime).iter().map(|s| s.unsigned_abs()).max().unwrap() as f32 / i16::MAX as f32;
            assert!(peak <= PEAK + 0.01, "{chime:?} peak {peak}");
            assert!(peak >= PEAK - 0.02, "{chime:?} should reach its level, got {peak}");
        }
    }

    #[test]
    fn starts_and_ends_silent_so_it_cannot_click() {
        for chime in [Chime::Success, Chime::Error] {
            let s = samples(chime);
            assert_eq!(s[0], 0);
            assert!(s[s.len() - 1].abs() < 8, "ends near zero");
            // The last few milliseconds are already faded.
            let tail = &s[s.len() - 200..];
            assert!(tail.iter().all(|v| v.unsigned_abs() < 200), "tail is faded out");
        }
    }

    #[test]
    fn is_brief_and_the_two_chimes_differ() {
        let (a, b) = (samples(Chime::Success), samples(Chime::Error));
        assert!(a.len() as f32 / SAMPLE_RATE as f32 <= 0.6, "brief");
        assert_ne!(a, b);
    }
}
