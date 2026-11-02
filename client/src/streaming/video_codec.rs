#![allow(dead_code)]
//! Encodage vidéo H.264 par processus ffmpeg (NVENC matériel, repli logiciel).
//!
//! Le JPEG intra-image ne tient pas 1080p60 dans une bande passante domestique :
//! on lui préfère un encodeur inter-images à débit constant. ffmpeg est piloté en
//! fils (rawvideo brut sur stdin, flux H.264 élémentaire sur stdout), ce qui évite
//! d'écrire un pipeline Media Foundation complet.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Chunk maximum produit par l'encodeur : assez petit pour une trame chiffrée.
pub const CHUNK_SIZE: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// H.264 matériel (NVENC / AMF / QuickSync selon la build ffmpeg).
    Nvenc,
    /// H.264 logiciel (libx264 ultrafast).
    Software,
    /// Repli : JPEG intra-image, l'ancien chemin.
    Jpeg,
}

impl Codec {
    pub fn as_str(&self) -> &'static str {
        match self {
            Codec::Nvenc => "h264_nvenc",
            Codec::Software => "libx264",
            Codec::Jpeg => "jpeg",
        }
    }
}

/// Cherche ffmpeg.exe à côté de l'application, dans `bin/`, puis dans le PATH.
pub fn find_ffmpeg() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("ffmpeg.exe"),
        PathBuf::from("bin/ffmpeg.exe"),
        PathBuf::from("ffmpeg"),
        PathBuf::from("bin/ffmpeg"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.insert(0, dir.join("ffmpeg.exe"));
            candidates.insert(1, dir.join("bin").join("ffmpeg.exe"));
            candidates.insert(2, dir.join("redist").join("ffmpeg.exe"));
        }
    }
    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // PATH
    let names: &[&str] = if cfg!(windows) {
        &["ffmpeg.exe"]
    } else {
        &["ffmpeg"]
    };
    for name in names {
        if let Ok(out) = Command::new("where").arg(name).output() {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
        if let Ok(out) = Command::new("which").arg(name).output() {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
    }
    None
}

/// Vérifie que ffmpeg sait encoder avec ce codec (sonde de 0,2 s).
pub fn probe_encoder(ffmpeg: &PathBuf, encoder: &str) -> bool {
    let status = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "nullsrc=s=320x240:r=30:d=0.2",
            "-c:v",
            encoder,
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(status, Ok(s) if s.success())
}

/// Arguments d'encodage, sans le chemin de l'exécutable (testable sans ffmpeg).
pub fn encoder_args(codec: Codec, width: u32, height: u32, fps: u32, bitrate_kbps: u32) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "-s".into(),
        format!("{}x{}", width, height),
        "-r".into(),
        fps.to_string(),
        "-i".into(),
        "pipe:0".into(),
        "-c:v".into(),
        codec.as_str().into(),
    ];

    match codec {
        Codec::Nvenc => {
            args.extend([
                "-preset".into(),
                "p3".into(),
                "-tune".into(),
                "ll".into(),
                "-rc".into(),
                "cbr".into(),
                "-zerolatency".into(),
                "1".into(),
            ]);
        }
        Codec::Software => {
            args.extend(["-preset".into(), "ultrafast".into(), "-tune".into(), "zerolatency".into()]);
        }
        Codec::Jpeg => {}
    }

    args.extend([
        "-b:v".into(),
        format!("{}k", bitrate_kbps.max(1_000)),
        "-maxrate".into(),
        format!("{}k", bitrate_kbps.max(1_000)),
        "-bufsize".into(),
        format!("{}k", bitrate_kbps.max(1_000)),
        "-g".into(),
        fps.max(1).to_string(),
        "-bf".into(),
        "0".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-f".into(),
        "h264".into(),
        "pipe:1".into(),
    ]);
    args
}

/// Arguments du décodeur côté invité.
pub fn decoder_args() -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-probesize".into(),
        "32".into(),
        "-analyzeduration".into(),
        "0".into(),
        "-fflags".into(),
        "nobuffer".into(),
        "-flags".into(),
        "low_delay".into(),
        "-f".into(),
        "h264".into(),
        "-i".into(),
        "pipe:0".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "pipe:1".into(),
    ]
}

pub fn spawn_hidden(ffmpeg: &PathBuf, args: &[String]) -> Option<Child> {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.spawn().ok()
}

/// Encodeur H.264 piloté par un processus ffmpeg.
pub struct VideoEncoder {
    child: Child,
    stdin: ChildStdin,
    rx: crossbeam_channel::Receiver<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub codec: Codec,
}

impl VideoEncoder {
    /// Démarre ffmpeg et le thread qui remonte les chunks H.264.
    pub fn start(
        ffmpeg: &PathBuf,
        codec: Codec,
        width: u32,
        height: u32,
        fps: u32,
        bitrate_kbps: u32,
    ) -> Option<Self> {
        let args = encoder_args(codec, width, height, fps, bitrate_kbps);
        let mut child = spawn_hidden(ffmpeg, &args)?;
        let stdin = child.stdin.take()?;
        let mut stdout = child.stdout.take()?;
        let (tx, rx) = crossbeam_channel::bounded(256);

        thread::spawn(move || {
            let mut buf = vec![0u8; CHUNK_SIZE];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Some(Self {
            child,
            stdin,
            rx,
            width,
            height,
            codec,
        })
    }

    /// Pousse une image RGB brute. `false` = encodeur mort, il faut repartir en JPEG.
    pub fn write_frame(&mut self, rgb: &[u8]) -> bool {
        let expected = (self.width * self.height * 3) as usize;
        if rgb.len() != expected {
            return false;
        }
        match self.stdin.write_all(rgb) {
            Ok(()) => true,
            Err(_) => {
                // Le processus a disparu (codec refusé par le GPU, etc.).
                matches!(self.child.try_wait(), Ok(None)) && false
            }
        }
    }

    /// Chunks déjà encodés, à envoyer tels quels.
    pub fn drain(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Ok(chunk) = self.rx.try_recv() {
            out.push(chunk);
            if out.len() >= 64 {
                break;
            }
        }
        out
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Décodeur côté invité : RGBA brut sur stdout, une image à la fois.
pub struct VideoDecoder {
    child: Child,
    stdin: ChildStdin,
}

impl VideoDecoder {
    pub fn start(ffmpeg: &PathBuf) -> Option<Self> {
        let args = decoder_args();
        let mut child = spawn_hidden(ffmpeg, &args)?;
        let stdin = child.stdin.take()?;
        Some(Self { child, stdin })
    }

    pub fn take_stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.child.stdout.take()
    }

    pub fn feed(&mut self, chunk: &[u8]) -> bool {
        self.stdin.write_all(chunk).is_ok()
    }
}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Lit `width * height * 4` octets (une image RGBA) sur le flux du décodeur.
pub fn read_rgba_frame(
    reader: &mut impl Read,
    width: usize,
    height: usize,
    running: &Arc<AtomicBool>,
) -> Option<Vec<u8>> {
    let size = width * height * 4;
    let mut buf = vec![0u8; size];
    let mut filled = 0usize;
    while filled < size {
        if !running.load(Ordering::Relaxed) {
            return None;
        }
        match reader.read(&mut buf[filled..]) {
            Ok(0) => return None,
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoder_args_are_cbr_and_keyframe_sized() {
        let args = encoder_args(Codec::Software, 1920, 1080, 60, 40_000);
        let joined = args.join(" ");
        assert!(joined.contains("-f rawvideo"));
        assert!(joined.contains("-s 1920x1080"));
        assert!(joined.contains("-r 60"));
        assert!(joined.contains("-b:v 40000k"));
        assert!(joined.contains("-maxrate 40000k"));
        assert!(joined.contains("-bufsize 40000k"));
        assert!(joined.contains("-bf 0"));
        assert!(joined.ends_with("-f h264 pipe:1"));
    }

    #[test]
    fn nvenc_uses_low_latency_preset() {
        let args = encoder_args(Codec::Nvenc, 1280, 720, 60, 20_000).join(" ");
        assert!(args.contains("-c:v h264_nvenc"));
        assert!(args.contains("-tune ll"));
        assert!(args.contains("-rc cbr"));
    }

    #[test]
    fn decoder_asks_for_rgba_frames() {
        let args = decoder_args().join(" ");
        assert!(args.contains("-f h264"));
        assert!(args.contains("-pix_fmt rgba"));
        assert!(args.contains("nobuffer"));
    }
}
