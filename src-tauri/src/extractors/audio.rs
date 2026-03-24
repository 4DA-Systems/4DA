/// Audio transcription extraction
///
/// Transcribes audio files to text using Whisper.
///
/// CURRENT STATUS: Stubbed - requires system dependencies
///
/// To enable audio transcription:
/// 1. Install LLVM/Clang (for bindgen)
/// 2. Uncomment `whisper-rs = "0.14"` in Cargo.toml
/// 3. Download a Whisper model from:
///    https://huggingface.co/ggerganov/whisper.cpp/tree/main
///    Place ggml-base.bin in %LOCALAPPDATA%\4da\models\ or ~/.4da/models/
///
/// Alternatively, wait for pure Rust Whisper implementations to mature
/// (whisper-apr, candle-whisper)
use super::{DocumentExtractor, ExtractedDocument};
use crate::error::Result;
use std::path::Path;

pub struct AudioExtractor;

impl AudioExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AudioExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentExtractor for AudioExtractor {
    fn supported_extensions(&self) -> &[&str] {
        &["wav", "mp3", "ogg", "m4a", "flac", "aac"]
    }

    fn extract(&self, path: &Path) -> Result<ExtractedDocument> {
        Err(format!(
            "Audio transcription not available.\n\
             File: {path:?}\n\n\
             To enable, install LLVM and enable whisper-rs in Cargo.toml.\n\
             See src/extractors/audio.rs header for detailed instructions."
        )
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_supported_extensions() {
        let extractor = AudioExtractor::new();
        let exts = extractor.supported_extensions();
        assert!(exts.contains(&"wav"));
        assert!(exts.contains(&"mp3"));
        assert!(exts.contains(&"ogg"));
    }

    #[test]
    fn test_audio_can_handle() {
        let extractor = AudioExtractor::new();
        assert!(extractor.can_handle(Path::new("test.wav")));
        assert!(extractor.can_handle(Path::new("test.MP3")));
        assert!(!extractor.can_handle(Path::new("test.pdf")));
    }

    #[test]
    fn test_audio_not_implemented() {
        let extractor = AudioExtractor::new();
        let result = extractor.extract(Path::new("/some/audio.wav"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not available"));
    }
}
