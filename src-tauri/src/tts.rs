//! Text-to-Speech module for 4DA Audio Briefings
//!
//! Uses platform-native TTS engines:
//! - Windows: PowerShell with System.Speech.Synthesis
//! - macOS: `say` command
//! - Linux: `espeak` or `festival`

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioBriefingStatus {
    pub available: bool,
    pub file_path: Option<String>,
    pub duration_seconds: Option<u32>,
    pub generated_at: Option<String>,
    pub tts_engine: String,
}

// ============================================================================
// TTS Engine
// ============================================================================

/// Detect available TTS engine on this platform
fn detect_tts_engine() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "sapi"
    }
    #[cfg(target_os = "macos")]
    {
        "say"
    }
    #[cfg(target_os = "linux")]
    {
        "espeak"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "none"
    }
}

/// Strip markdown formatting from briefing text for TTS
fn strip_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim();
        // Skip empty lines
        if trimmed.is_empty() {
            result.push(' ');
            continue;
        }
        // Strip markdown headers
        let clean = if trimmed.starts_with('#') {
            trimmed.trim_start_matches('#').trim()
        } else {
            trimmed
        };
        // Strip bold/italic markers
        let clean = clean.replace("**", "").replace('*', "").replace('_', " ");
        // Strip links: [text](url) -> text
        let clean = strip_markdown_links(&clean);
        // Strip bullet points
        let clean = clean.trim_start_matches("- ").trim_start_matches("• ");
        result.push_str(clean);
        result.push_str(". ");
    }
    result
}

fn strip_markdown_links(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            // Capture link text
            let mut link_text = String::new();
            for inner in chars.by_ref() {
                if inner == ']' {
                    break;
                }
                link_text.push(inner);
            }
            // Skip (url) part
            if chars.peek() == Some(&'(') {
                chars.next(); // consume '('
                for inner in chars.by_ref() {
                    if inner == ')' {
                        break;
                    }
                }
            }
            result.push_str(&link_text);
        } else {
            result.push(c);
        }
    }
    result
}

/// Get the audio output directory
fn get_audio_dir() -> Result<PathBuf, String> {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.push("data");
    base.push("audio");
    std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create audio dir: {}", e))?;
    Ok(base)
}

/// Generate audio briefing using platform TTS
pub fn generate_audio(text: &str, max_duration_seconds: u32) -> Result<PathBuf, String> {
    let engine = detect_tts_engine();
    if engine == "none" {
        return Err("No TTS engine available on this platform".to_string());
    }

    let clean_text = strip_markdown(text);
    // Limit text length based on approximate speaking rate (~150 words/min)
    let max_words = (max_duration_seconds as usize * 150) / 60;
    let truncated: String = clean_text
        .split_whitespace()
        .take(max_words)
        .collect::<Vec<_>>()
        .join(" ");

    let audio_dir = get_audio_dir()?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let output_path = audio_dir.join(format!("briefing_{}.wav", timestamp));

    info!(target: "4da::tts", engine, words = truncated.split_whitespace().count(), "Generating audio briefing");

    match engine {
        "sapi" => generate_with_sapi(&truncated, &output_path),
        "say" => generate_with_say(&truncated, &output_path),
        "espeak" => generate_with_espeak(&truncated, &output_path),
        _ => Err("Unsupported TTS engine".to_string()),
    }?;

    // Clean up old audio files (keep last 5)
    cleanup_old_audio(&audio_dir);

    Ok(output_path)
}

#[cfg(target_os = "windows")]
fn generate_with_sapi(text: &str, output_path: &PathBuf) -> Result<(), String> {
    let escaped = text.replace('\'', "''").replace('"', "`\"");
    let output_str = output_path.to_string_lossy();

    let script = format!(
        r#"Add-Type -AssemblyName System.Speech;
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer;
$synth.SetOutputToWaveFile('{}');
$synth.Speak('{}');
$synth.Dispose();"#,
        output_str, escaped
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell TTS: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell TTS failed: {}", stderr));
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn generate_with_sapi(_text: &str, _output_path: &PathBuf) -> Result<(), String> {
    Err("SAPI not available on this platform".to_string())
}

#[cfg(target_os = "macos")]
fn generate_with_say(text: &str, output_path: &PathBuf) -> Result<(), String> {
    let output = std::process::Command::new("say")
        .args(["-o", &output_path.to_string_lossy(), text])
        .output()
        .map_err(|e| format!("Failed to run say: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("say command failed: {}", stderr));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn generate_with_say(_text: &str, _output_path: &PathBuf) -> Result<(), String> {
    Err("say not available on this platform".to_string())
}

fn generate_with_espeak(text: &str, output_path: &PathBuf) -> Result<(), String> {
    let output = std::process::Command::new("espeak")
        .args(["-w", &output_path.to_string_lossy(), text])
        .output()
        .map_err(|e| format!("Failed to run espeak: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("espeak failed: {}", stderr));
    }
    Ok(())
}

fn cleanup_old_audio(audio_dir: &PathBuf) {
    if let Ok(entries) = std::fs::read_dir(audio_dir) {
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "wav")
                    .unwrap_or(false)
            })
            .map(|e| e.path())
            .collect();

        files.sort_by(|a, b| {
            b.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .cmp(
                    &a.metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                )
        });

        // Keep last 5 files
        for old_file in files.into_iter().skip(5) {
            if let Err(e) = std::fs::remove_file(&old_file) {
                warn!(target: "4da::tts", path = ?old_file, error = %e, "Failed to clean up old audio");
            }
        }
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub async fn generate_audio_briefing(_app: tauri::AppHandle) -> Result<String, String> {
    // Get the latest briefing text from the digest system
    let briefing_text = crate::digest_commands::get_latest_briefing_text()
        .ok_or_else(|| "No briefing available. Generate an AI briefing first.".to_string())?;

    let max_duration = {
        let settings = crate::get_settings_manager().lock();
        settings.get().audio_briefing.max_duration_seconds
    };

    let path = tokio::task::spawn_blocking(move || generate_audio(&briefing_text, max_duration))
        .await
        .map_err(|e| format!("TTS task failed: {}", e))??;

    let path_str = path.to_string_lossy().to_string();
    info!(target: "4da::tts", path = %path_str, "Audio briefing generated");

    Ok(path_str)
}

#[tauri::command]
pub fn get_audio_briefing_status() -> Result<AudioBriefingStatus, String> {
    let engine = detect_tts_engine();
    let audio_dir = get_audio_dir().ok();

    let latest = audio_dir.and_then(|dir| {
        std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "wav")
                    .unwrap_or(false)
            })
            .max_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            })
            .map(|e| e.path())
    });

    Ok(AudioBriefingStatus {
        available: engine != "none",
        file_path: latest.as_ref().map(|p| p.to_string_lossy().to_string()),
        duration_seconds: None,
        generated_at: latest.and_then(|p| {
            p.metadata().ok().and_then(|m| m.modified().ok()).map(|t| {
                chrono::DateTime::<chrono::Utc>::from(t)
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string()
            })
        }),
        tts_engine: engine.to_string(),
    })
}

#[tauri::command]
pub fn get_audio_file_path(file_name: String) -> Result<String, String> {
    let audio_dir = get_audio_dir()?;
    let path = audio_dir.join(&file_name);
    if path.exists() {
        Ok(path.to_string_lossy().to_string())
    } else {
        Err(format!("Audio file not found: {}", file_name))
    }
}
