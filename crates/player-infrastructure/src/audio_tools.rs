//! Offline audio work is deliberately separate from the playback engine. Every
//! subprocess receives an argument vector, owns its pipes, and can be cancelled.
use crate::InfraError;
use lofty::{
    config::WriteOptions,
    file::{AudioFile, TaggedFileExt},
    tag::{ItemKey, Tag},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const FFMPEG_WINDOWS_SHA256: &str =
    "3256173f3f8bffd7df12227c68adf68025edb1832273a9530688a7bb1ed8edec";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct AudioTools {
    executable: PathBuf,
}

#[derive(Debug, Clone, Copy)]
pub struct GainResult {
    pub gain: f64,
    pub peak: f64,
}

struct SiblingFile {
    path: PathBuf,
}
impl Drop for SiblingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn sibling(path: &Path) -> Result<SiblingFile, InfraError> {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for _ in 0..32 {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("tmp");
        let temporary = directory.join(format!(
            ".audio-player-{}-{now}-{nonce}.{extension}",
            std::process::id()
        ));
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(_) => return Ok(SiblingFile { path: temporary }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(InfraError::new(
        "export_error",
        "Could not reserve temporary output",
    ))
}

fn cancelled() -> InfraError {
    InfraError::new("cancelled", "Audio task cancelled")
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), InfraError> {
    if cancel.load(Ordering::Relaxed) {
        Err(cancelled())
    } else {
        Ok(())
    }
}
fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut tail = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    tail.extend_from_slice(&chunk[..n]);
                    if tail.len() > 65_536 {
                        tail.drain(..tail.len() - 65_536);
                    }
                }
            }
        }
        String::from_utf8_lossy(&tail).into_owned()
    })
}
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

impl AudioTools {
    /// Windows binaries are a pinned, verified build. Other platforms use an
    /// explicit executable supplied by the host rather than a shell command.
    pub fn new(executable: PathBuf) -> Result<Self, InfraError> {
        if !executable.is_file() {
            return Err(InfraError::new(
                "audio_tools_missing",
                "FFmpeg runtime is not installed",
            ));
        }
        #[cfg(windows)]
        {
            let mut file = File::open(&executable)?;
            let mut hash = Sha256::new();
            let mut bytes = [0_u8; 65_536];
            loop {
                let n = file.read(&mut bytes)?;
                if n == 0 {
                    break;
                }
                hash.update(&bytes[..n]);
            }
            if format!("{:x}", hash.finalize()) != FFMPEG_WINDOWS_SHA256 {
                return Err(InfraError::new(
                    "audio_tools_invalid",
                    "FFmpeg runtime failed SHA256 verification",
                ));
            }
        }
        Ok(Self { executable })
    }
    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command.args(["-hide_banner", "-nostdin", "-loglevel", "info"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
    }
    fn run(
        &self,
        args: &[String],
        seconds: f64,
        cancel: &AtomicBool,
        progress: &mut impl FnMut(f64),
    ) -> Result<String, InfraError> {
        check_cancel(cancel)?;
        let mut child = self
            .command()
            .args(["-progress", "pipe:1", "-stats_period", "0.2"])
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stderr = drain(child.stderr.take().unwrap());
        let output = child.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if let Some(value) = line
                    .strip_prefix("out_time_us=")
                    .and_then(|v| v.parse::<f64>().ok())
                {
                    let _ = send.send(value / 1_000_000.0);
                }
            }
        });
        let status = loop {
            if cancel.load(Ordering::Relaxed) {
                stop(&mut child);
                let _ = reader.join();
                let _ = stderr.join();
                return Err(cancelled());
            }
            while let Ok(time) = receive.try_recv() {
                if seconds > 0.0 {
                    progress((time / seconds).clamp(0.0, 0.99));
                }
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            std::thread::sleep(Duration::from_millis(40));
        };
        let _ = reader.join();
        let log = stderr.join().unwrap_or_default();
        if !status.success() {
            return Err(InfraError::new("audio_processing", log));
        }
        check_cancel(cancel)?;
        progress(1.0);
        Ok(log)
    }

    /// Concatenate decoded, identically formatted PCM, then run ONE ReplayGain
    /// scanner over the complete stream. This is real album/folder analysis,
    /// never an average of logarithmic per-track gain numbers.
    pub fn analyze(
        &self,
        inputs: &[(PathBuf, f64)],
        cancel: &AtomicBool,
        mut progress: impl FnMut(f64),
    ) -> Result<GainResult, InfraError> {
        if inputs.is_empty() {
            return Err(InfraError::new("invalid_input", "No audio selected"));
        }
        let mut analyzer = self
            .command()
            .args([
                "-f",
                "f32le",
                "-ar",
                "44100",
                "-ac",
                "2",
                "-i",
                "pipe:0",
                "-af",
                "replaygain",
                "-f",
                "null",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let log_reader = drain(analyzer.stderr.take().unwrap());
        let mut destination = analyzer.stdin.take().unwrap();
        let total = inputs
            .iter()
            .map(|(_, duration)| duration.max(0.0))
            .sum::<f64>();
        let mut decoded = 0_u64;
        let result = (|| -> Result<(), InfraError> {
            for (input, _) in inputs {
                check_cancel(cancel)?;
                let mut decoder = self
                    .command()
                    .arg("-i")
                    .arg(input)
                    .args([
                        "-vn", "-sn", "-dn", "-ar", "44100", "-ac", "2", "-f", "f32le", "pipe:1",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;
                let decoder_log = drain(decoder.stderr.take().unwrap());
                let mut source = decoder.stdout.take().unwrap();
                let mut buffer = [0_u8; 65_536];
                let copy = (|| -> Result<(), InfraError> {
                    loop {
                        check_cancel(cancel)?;
                        let n = source.read(&mut buffer)?;
                        if n == 0 {
                            break;
                        }
                        destination.write_all(&buffer[..n])?;
                        decoded += n as u64;
                        if total > 0.0 {
                            progress((decoded as f64 / (44_100.0 * 8.0) / total).clamp(0.0, 0.99));
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = copy {
                    stop(&mut decoder);
                    let _ = decoder_log.join();
                    return Err(error);
                }
                let status = decoder.wait()?;
                let log = decoder_log.join().unwrap_or_default();
                if !status.success() {
                    return Err(InfraError::new("audio_processing", log));
                }
            }
            Ok(())
        })();
        drop(destination);
        if let Err(error) = result {
            stop(&mut analyzer);
            let _ = log_reader.join();
            return Err(error);
        }
        let status = analyzer.wait()?;
        let log = log_reader.join().unwrap_or_default();
        if !status.success() {
            return Err(InfraError::new("audio_processing", log));
        }
        check_cancel(cancel)?;
        let result = parse_replaygain(&log)?;
        progress(1.0);
        Ok(result)
    }

    // Keep explicit edit and process parameters at this adapter boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn export_selection(
        &self,
        input: &Path,
        output: &Path,
        start: f64,
        end: f64,
        remove: bool,
        duration: f64,
        cancel: &AtomicBool,
        progress: impl FnMut(f64),
    ) -> Result<(), InfraError> {
        self.export_inner(
            input, output, start, end, remove, duration, None, cancel, progress,
        )
    }
    pub fn preview_selection(
        &self,
        input: &Path,
        output: &Path,
        start: f64,
        end: f64,
        remove: bool,
        duration: f64,
    ) -> Result<(), InfraError> {
        self.export_inner(
            input,
            output,
            start,
            end,
            remove,
            duration,
            Some(90.0),
            &AtomicBool::new(false),
            |_| {},
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn export_inner(
        &self,
        input: &Path,
        output: &Path,
        start: f64,
        end: f64,
        remove: bool,
        duration: f64,
        limit: Option<f64>,
        cancel: &AtomicBool,
        mut progress: impl FnMut(f64),
    ) -> Result<(), InfraError> {
        validate_selection(start, end, duration, remove)?;
        reject_source_output(input, output)?;
        let extension = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let (format, codec) = match extension.as_str() {
            "flac" => ("flac", "flac"),
            "wav" => (
                "wav",
                if limit.is_some() {
                    "pcm_s16le"
                } else {
                    "pcm_s24le"
                },
            ),
            _ => {
                return Err(InfraError::new(
                    "invalid_input",
                    "Choose FLAC or WAV for edited copies",
                ))
            }
        };
        let temporary = sibling(output)?;
        let filter = selection_filter(start, end, remove, duration);
        let mut args = vec![
            "-y".into(),
            "-i".into(),
            input.to_string_lossy().into_owned(),
            "-filter_complex".into(),
            filter,
            "-map".into(),
            "[out]".into(),
            "-map_metadata".into(),
            "0".into(),
            "-c:a".into(),
            codec.into(),
        ];
        if let Some(seconds) = limit {
            args.extend([
                "-t".into(),
                seconds.to_string(),
                "-ac".into(),
                "2".into(),
                "-ar".into(),
                "44100".into(),
            ]);
        }
        args.extend([
            "-f".into(),
            format.into(),
            temporary.path.to_string_lossy().into_owned(),
        ]);
        let length = if remove {
            duration - (end - start)
        } else {
            end - start
        };
        self.run(
            &args,
            limit.map_or(length, |l| length.min(l)),
            cancel,
            &mut progress,
        )?;
        check_cancel(cancel)?;
        // Recheck after processing: symlinks and aliases must never replace the
        // source. Publish only a complete, durable sibling on the same volume.
        reject_source_output(input, output)?;
        OpenOptions::new()
            .write(true)
            .open(&temporary.path)?
            .sync_all()?;
        fs::rename(&temporary.path, output)?;
        Ok(())
    }

    pub fn waveform(&self, input: &Path, duration: f64) -> Result<Vec<f64>, InfraError> {
        if !duration.is_finite() || duration <= 0.0 || duration > 43_200.0 {
            return Err(InfraError::new(
                "invalid_input",
                "Waveform requires an audio duration of at most 12 hours",
            ));
        }
        let mut child = self
            .command()
            .arg("-i")
            .arg(input)
            .args([
                "-vn", "-sn", "-dn", "-ac", "1", "-ar", "8000", "-f", "f32le", "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let log = drain(child.stderr.take().unwrap());
        let mut source = BufReader::new(child.stdout.take().unwrap());
        let mut peaks = vec![0_f64; 1000];
        let mut index = 0_u64;
        let mut sample = [0_u8; 4];
        loop {
            match source.read_exact(&mut sample) {
                Ok(()) => {
                    let value = f32::from_le_bytes(sample) as f64;
                    let bucket = ((index as f64 / (duration * 8000.0)) * 1000.0) as usize;
                    if bucket < 1000 && value.is_finite() {
                        peaks[bucket] = peaks[bucket].max(value.abs());
                    }
                    index += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => {
                    stop(&mut child);
                    let _ = log.join();
                    return Err(e.into());
                }
            }
        }
        let status = child.wait()?;
        let diagnostic = log.join().unwrap_or_default();
        if !status.success() {
            return Err(InfraError::new("audio_processing", diagnostic));
        }
        let maximum = peaks.iter().copied().fold(0.0_f64, f64::max);
        if maximum > 0.0 {
            for peak in &mut peaks {
                *peak /= maximum;
            }
        }
        Ok(peaks)
    }

    pub fn validate_image(&self, path: &Path) -> Result<(), InfraError> {
        let args = vec![
            "-max_pixels".into(),
            "16777216".into(),
            "-i".into(),
            path.to_string_lossy().into_owned(),
            "-frames:v".into(),
            "1".into(),
            "-f".into(),
            "null".into(),
            "-".into(),
        ];
        self.run(&args, 0.0, &AtomicBool::new(false), &mut |_| {})
            .map(|_| ())
    }
}

pub fn parse_replaygain(log: &str) -> Result<GainResult, InfraError> {
    let number = |key: &str| {
        log.lines().rev().find_map(|line| {
            line.split_once(key)
                .and_then(|(_, tail)| tail.split_whitespace().next())
                .and_then(|s| s.parse::<f64>().ok())
        })
    };
    let gain = number("track_gain = ").filter(|v| v.is_finite() && (-100.0..=100.0).contains(v));
    let peak = number("track_peak = ").filter(|v| v.is_finite() && (0.0..=100.0).contains(v));
    match (gain, peak) {
        (Some(gain), Some(peak)) => Ok(GainResult { gain, peak }),
        _ => Err(InfraError::new(
            "analysis_error",
            "FFmpeg did not return valid ReplayGain values",
        )),
    }
}
fn validate_selection(start: f64, end: f64, duration: f64, remove: bool) -> Result<(), InfraError> {
    if !start.is_finite()
        || !end.is_finite()
        || !duration.is_finite()
        || duration <= 0.0
        || start < 0.0
        || end <= start
        || end > duration + 0.001
        || (remove && start == 0.0 && end >= duration)
    {
        return Err(InfraError::new(
            "invalid_input",
            "Select a nonempty audio range within the song",
        ));
    }
    Ok(())
}
fn selection_filter(start: f64, end: f64, remove: bool, duration: f64) -> String {
    if !remove {
        format!("[0:a]atrim=start={start:.9}:end={end:.9},asetpts=PTS-STARTPTS[out]")
    } else if start == 0.0 {
        format!("[0:a]atrim=start={end:.9},asetpts=PTS-STARTPTS[out]")
    } else if end >= duration {
        format!("[0:a]atrim=end={start:.9},asetpts=PTS-STARTPTS[out]")
    } else {
        format!("[0:a]asplit=2[a][b];[a]atrim=end={start:.9},asetpts=PTS-STARTPTS[left];[b]atrim=start={end:.9},asetpts=PTS-STARTPTS[right];[left][right]concat=n=2:v=0:a=1[out]")
    }
}
fn reject_source_output(input: &Path, output: &Path) -> Result<(), InfraError> {
    let source = fs::canonicalize(input)?;
    let destination = if output.exists() {
        fs::canonicalize(output)?
    } else {
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::canonicalize(parent)?.join(
            output
                .file_name()
                .ok_or_else(|| InfraError::new("invalid_input", "Output must name a file"))?,
        )
    };
    let same = if cfg!(windows) {
        source
            .to_string_lossy()
            .eq_ignore_ascii_case(&destination.to_string_lossy())
    } else {
        source == destination
    };
    if same {
        return Err(InfraError::new(
            "invalid_input",
            "The edited copy must use a different file from the original",
        ));
    }
    // Canonical paths catch symlinks; hard links require file identity as well.
    #[cfg(windows)]
    if output.exists() && same_windows_file(input, output)? {
        return Err(InfraError::new(
            "invalid_input",
            "Output is a hard link to the original",
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn same_windows_file(a: &Path, b: &Path) -> Result<bool, InfraError> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Information {
        attributes: u32,
        created: FileTime,
        accessed: FileTime,
        written: FileTime,
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandle(handle: *mut c_void, info: *mut Information) -> i32;
    }
    let a = File::open(a)?;
    let b = File::open(b)?;
    let mut ai = Information::default();
    let mut bi = Information::default();
    // SAFETY: Open file handles stay live and structures match BY_HANDLE_FILE_INFORMATION.
    if unsafe { GetFileInformationByHandle(a.as_raw_handle(), &mut ai) } == 0
        || unsafe { GetFileInformationByHandle(b.as_raw_handle(), &mut bi) } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(ai.volume == bi.volume && ai.index_high == bi.index_high && ai.index_low == bi.index_low)
}
/// Metadata writes are opt-in and transactional per file: edit a sibling copy,
/// read the values back with Lofty, then replace the original. Folder gain has
/// no interoperable tag and is intentionally stored only in the app database.
pub fn write_gain_tags(path: &Path, result: GainResult, album: bool) -> Result<(), InfraError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "flac" | "mp3" | "ogg" | "oga" | "m4a") {
        return Err(InfraError::new(
            "unsupported_tags",
            "ReplayGain tag writing supports FLAC, MP3, Ogg Vorbis and M4A",
        ));
    }
    let temporary = sibling(path)?;
    fs::copy(path, &temporary.path)?;
    let mut file = lofty::read_from_path(&temporary.path)
        .map_err(|e| InfraError::new("tag_error", e.to_string()))?;
    if file.primary_tag().is_none() {
        file.insert_tag(Tag::new(file.primary_tag_type()));
    }
    let tag = file
        .primary_tag_mut()
        .ok_or_else(|| InfraError::new("tag_error", "File has no writable metadata"))?;
    let (gain_key, peak_key) = if album {
        (ItemKey::ReplayGainAlbumGain, ItemKey::ReplayGainAlbumPeak)
    } else {
        (ItemKey::ReplayGainTrackGain, ItemKey::ReplayGainTrackPeak)
    };
    let gain = format!("{:+.2} dB", result.gain);
    let peak = format!("{:.8}", result.peak);
    tag.insert_text(gain_key, gain.clone());
    tag.insert_text(peak_key, peak.clone());
    file.save_to_path(&temporary.path, WriteOptions::default())
        .map_err(|e| InfraError::new("tag_error", e.to_string()))?;
    let verified = lofty::read_from_path(&temporary.path)
        .map_err(|e| InfraError::new("tag_error", e.to_string()))?;
    let tags = verified
        .primary_tag()
        .ok_or_else(|| InfraError::new("tag_error", "Written tags could not be read back"))?;
    if tags.get_string(gain_key) != Some(gain.as_str())
        || tags.get_string(peak_key) != Some(peak.as_str())
    {
        return Err(InfraError::new(
            "tag_error",
            "ReplayGain metadata verification failed",
        ));
    }
    OpenOptions::new()
        .write(true)
        .open(&temporary.path)?
        .sync_all()?;
    fs::rename(&temporary.path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_only_finite_complete_scanner_results() {
        let value = parse_replaygain(
            "[ReplayGain] track_gain = -3.45 dB\n[ReplayGain] track_peak = 0.998765\n",
        )
        .unwrap();
        assert_eq!(value.gain, -3.45);
        assert_eq!(value.peak, 0.998765);
        assert!(parse_replaygain("track_gain = NaN dB\ntrack_peak = 1").is_err());
        assert!(parse_replaygain("track_gain = 2 dB").is_err());
    }
    #[test]
    fn source_and_hard_link_are_rejected_before_any_write() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("song.wav");
        fs::write(&original, b"original").unwrap();
        assert!(reject_source_output(&original, &original).is_err());
        let link = dir.path().join("alias.wav");
        fs::hard_link(&original, &link).unwrap();
        #[cfg(windows)]
        assert!(reject_source_output(&original, &link).is_err());
        assert_eq!(fs::read(original).unwrap(), b"original");
    }
    #[test]
    fn edit_ranges_reject_empty_full_removal_and_nonfinite_values() {
        assert!(validate_selection(0.0, 10.0, 10.0, true).is_err());
        assert!(validate_selection(f64::NAN, 2.0, 10.0, false).is_err());
        assert!(validate_selection(2.0, 5.0, 10.0, true).is_ok());
        assert!(selection_filter(2.0, 5.0, true, 10.0).contains("concat=n=2"));
    }

    fn runtime() -> AudioTools {
        AudioTools::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../src-tauri/resources/ffmpeg/ffmpeg.exe"),
        )
        .unwrap()
    }
    fn tone(tool: &AudioTools, path: &Path, volume: f64, rate: u32, codec: &str) {
        let status = tool
            .command()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:sample_rate={rate}:duration=3"),
                "-af",
                &format!("volume={volume}"),
                "-c:a",
                codec,
            ])
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }
    fn audio_digest(tool: &AudioTools, path: &Path) -> Vec<u8> {
        let output = tool
            .command()
            .arg("-i")
            .arg(path)
            .args(["-map", "0:a", "-f", "s16le", "pipe:1"])
            .output()
            .unwrap();
        assert!(output.status.success());
        Sha256::digest(output.stdout).to_vec()
    }

    #[test]
    #[ignore = "requires bundled pinned Windows FFmpeg runtime"]
    fn native_audio_tools_analyze_true_groups_and_export_precise_copies() {
        let tool = runtime();
        let directory = tempfile::tempdir().unwrap();
        let loud = directory.path().join("Über 🎵 loud.wav");
        let quiet = directory.path().join("quiet.wav");
        tone(&tool, &loud, 4.0, 48_000, "pcm_s16le");
        tone(&tool, &quiet, 0.1, 16_000, "pcm_s16le");
        let original = fs::read(&loud).unwrap();
        let cancel = AtomicBool::new(false);
        let a = tool
            .analyze(&[(loud.clone(), 3.0)], &cancel, |_| {})
            .unwrap();
        let b = tool
            .analyze(&[(quiet.clone(), 3.0)], &cancel, |_| {})
            .unwrap();
        let group = tool
            .analyze(
                &[(loud.clone(), 3.0), (quiet.clone(), 3.0)],
                &cancel,
                |_| {},
            )
            .unwrap();
        assert!(
            (group.gain - (a.gain + b.gain) / 2.0).abs() > 1.0,
            "group analysis must not average dB values: {a:?} {b:?} {group:?}"
        );
        let waveform = tool.waveform(&loud, 3.0).unwrap();
        assert_eq!(waveform.len(), 1000);
        assert!(waveform
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        assert!(waveform.iter().any(|v| *v > 0.99));
        let keep = directory.path().join("keep.flac");
        tool.export_selection(&loud, &keep, 0.5, 1.5, false, 3.0, &cancel, |_| {})
            .unwrap();
        assert!(
            (lofty::read_from_path(&keep)
                .unwrap()
                .properties()
                .duration()
                .as_secs_f64()
                - 1.0)
                .abs()
                < 0.002
        );
        let remove = directory.path().join("remove.wav");
        tool.export_selection(&loud, &remove, 0.5, 1.5, true, 3.0, &cancel, |_| {})
            .unwrap();
        assert!(
            (lofty::read_from_path(&remove)
                .unwrap()
                .properties()
                .duration()
                .as_secs_f64()
                - 2.0)
                .abs()
                < 0.002
        );
        let preview = directory.path().join("preview.wav");
        tool.preview_selection(&loud, &preview, 0.5, 1.5, false, 3.0)
            .unwrap();
        assert!(preview.exists());
        assert_eq!(fs::read(&loud).unwrap(), original);
        let destination = directory.path().join("existing.flac");
        fs::write(&destination, b"prior complete output").unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            tool.export_selection(&loud, &destination, 0.5, 1.5, false, 3.0, &cancel, |_| {})
                .unwrap_err()
                .code,
            "cancelled"
        );
        assert_eq!(fs::read(destination).unwrap(), b"prior complete output");
        cancel.store(false, Ordering::Relaxed);
        let result = tool.analyze(&[(loud, 3.0), (quiet, 3.0)], &cancel, |p| {
            if p > 0.05 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(result.unwrap_err().code, "cancelled");
    }

    #[test]
    #[ignore = "requires bundled pinned Windows FFmpeg runtime"]
    fn native_replaygain_tag_roundtrips_preserve_decoded_audio_for_supported_formats() {
        let tool = runtime();
        let directory = tempfile::tempdir().unwrap();
        for (extension, codec) in [
            ("flac", "flac"),
            ("mp3", "libmp3lame"),
            ("ogg", "libvorbis"),
            ("m4a", "aac"),
        ] {
            let path = directory.path().join(format!("Tags – Grüße.{extension}"));
            tone(&tool, &path, 1.0, 44_100, codec);
            let before = audio_digest(&tool, &path);
            write_gain_tags(
                &path,
                GainResult {
                    gain: -3.25,
                    peak: 0.92345678,
                },
                false,
            )
            .unwrap();
            write_gain_tags(
                &path,
                GainResult {
                    gain: -1.75,
                    peak: 0.98765432,
                },
                true,
            )
            .unwrap();
            let file = lofty::read_from_path(&path).unwrap();
            let tag = file.primary_tag().unwrap();
            assert_eq!(
                tag.get_string(ItemKey::ReplayGainTrackGain),
                Some("-3.25 dB")
            );
            assert_eq!(
                tag.get_string(ItemKey::ReplayGainAlbumGain),
                Some("-1.75 dB")
            );
            assert_eq!(
                audio_digest(&tool, &path),
                before,
                "metadata rewrite changed decoded audio for {extension}"
            );
        }
    }
}
