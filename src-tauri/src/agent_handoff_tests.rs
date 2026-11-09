// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Hand-off URL building and every refusal. Detection that needs a real registry
//! or desktop is not unit-tested; its pure helpers are.
use super::*;

/// Reads this machine's real registrations; run by hand with `--ignored` to see
/// what a user's 4DA would offer. Opens nothing.
#[test]
#[ignore = "reads this machine's link handlers and PATH"]
fn live_detection_on_this_machine() {
    let found = detect::detect();
    println!("targets: {:?}", found.targets);
    println!("claude path: {:?}", found.claude_path);
    if found.targets.claude_code_route == ClaudeCodeRoute::Paste {
        assert!(
            found.claude_path.is_some(),
            "the paste route needs the program path"
        );
    }
}

fn validation_msg(r: Result<String>) -> String {
    match r {
        Err(FourDaError::Validation(m)) => m,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[test]
fn claude_url_without_cwd_encodes_the_prompt() {
    let url =
        build_handoff_url(HandoffTarget::ClaudeCode, "fix a&b #1 + c\nnext line", None).unwrap();
    assert_eq!(
        url,
        "claude-cli://open?q=fix%20a%26b%20%231%20%2B%20c%0Anext%20line"
    );
}

#[test]
fn claude_url_with_cwd_puts_an_encoded_absolute_path_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_string_lossy().to_string();
    let url = build_handoff_url(HandoffTarget::ClaudeCode, "hi", Some(&path)).unwrap();
    let expected = format!("claude-cli://open?cwd={}&q=hi", urlencoding::encode(&path));
    assert_eq!(url, expected);
    // The encoded path never carries a raw separator that would split the query.
    let cwd_part = url
        .trim_start_matches("claude-cli://open?cwd=")
        .split("&q=")
        .next()
        .unwrap();
    assert!(!cwd_part.contains('&') && !cwd_part.contains('/') && !cwd_part.contains('\\'));
}

#[test]
fn vscode_and_cursor_urls_use_their_documented_parameters() {
    assert_eq!(
        build_handoff_url(HandoffTarget::VscodeClaude, "review my changes", None).unwrap(),
        "vscode://anthropic.claude-code/open?prompt=review%20my%20changes"
    );
    assert_eq!(
        build_handoff_url(HandoffTarget::Cursor, "a&b", None).unwrap(),
        "cursor://anysphere.cursor-deeplink/prompt?text=a%26b"
    );
}

#[test]
fn cwd_is_ignored_for_agents_that_open_in_the_focused_window() {
    let url =
        build_handoff_url(HandoffTarget::VscodeClaude, "hi", Some("not even a path")).unwrap();
    assert_eq!(url, "vscode://anthropic.claude-code/open?prompt=hi");
}

#[test]
fn non_ascii_prompts_are_percent_encoded_as_utf8() {
    let url = build_handoff_url(HandoffTarget::ClaudeCode, "café", None).unwrap();
    assert_eq!(url, "claude-cli://open?q=caf%C3%A9");
}

#[test]
fn claude_prompt_cap_is_five_thousand_characters() {
    let at_cap = "a".repeat(CLAUDE_PROMPT_MAX_CHARS);
    assert!(build_handoff_url(HandoffTarget::ClaudeCode, &at_cap, None).is_ok());
    let over = "a".repeat(CLAUDE_PROMPT_MAX_CHARS + 1);
    let msg = validation_msg(build_handoff_url(HandoffTarget::ClaudeCode, &over, None));
    assert!(msg.contains("5000"), "{msg}");
}

#[test]
fn url_cap_holds_for_agents_without_a_prompt_cap() {
    // 4,000 characters, half of them spaces, encode to 8,000: with the prefix the
    // link is over the 8,000 cap.
    let msg = validation_msg(build_handoff_url(
        HandoffTarget::Cursor,
        &"a ".repeat(2_000),
        None,
    ));
    assert!(msg.contains("8000"), "{msg}");
    assert!(build_handoff_url(HandoffTarget::Cursor, &"a".repeat(7_000), None).is_ok());
}

#[test]
fn empty_and_whitespace_prompts_are_refused() {
    validation_msg(build_handoff_url(HandoffTarget::ClaudeCode, "", None));
    validation_msg(build_handoff_url(HandoffTarget::Cursor, " \n\t ", None));
}

#[test]
fn hidden_characters_in_the_prompt_are_refused() {
    for bad in [
        "a\u{0000}b",
        "a\rb",
        "a\u{202E}b",
        "a\u{200B}b",
        "a\u{2066}b",
        "a\u{FEFF}b",
        "a\u{001B}[2Jb",
    ] {
        let msg = validation_msg(build_handoff_url(HandoffTarget::ClaudeCode, bad, None));
        assert!(msg.contains("control"), "{bad:?}: {msg}");
    }
    // Ordinary formatting is fine.
    assert!(build_handoff_url(HandoffTarget::ClaudeCode, "line one\n\tline two", None).is_ok());
}

#[test]
fn relative_cwd_is_refused() {
    let msg = validation_msg(build_handoff_url(
        HandoffTarget::ClaudeCode,
        "hi",
        Some("projects/app"),
    ));
    assert!(msg.contains("absolute"), "{msg}");
}

#[test]
fn parent_dir_segments_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let sneaky = format!(
        "{}{}..{}",
        dir.path().display(),
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    );
    let msg = validation_msg(build_handoff_url(
        HandoffTarget::ClaudeCode,
        "hi",
        Some(&sneaky),
    ));
    assert!(msg.contains(".."), "{msg}");
}

#[test]
fn unc_and_network_paths_are_refused() {
    for unc in [
        r"\\server\share\repo",
        "//server/share/repo",
        r"\\?\C:\repo",
    ] {
        let msg = validation_msg(build_handoff_url(
            HandoffTarget::ClaudeCode,
            "hi",
            Some(unc),
        ));
        assert!(msg.contains("UNC"), "{unc}: {msg}");
    }
}

#[test]
fn hidden_characters_in_cwd_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let bad = format!("{}\u{202E}", dir.path().display());
    let msg = validation_msg(build_handoff_url(
        HandoffTarget::ClaudeCode,
        "hi",
        Some(&bad),
    ));
    assert!(msg.contains("control"), "{msg}");
}

#[test]
fn missing_folder_and_file_paths_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("does-not-exist");
    let msg = validation_msg(build_handoff_url(
        HandoffTarget::ClaudeCode,
        "hi",
        Some(&gone.to_string_lossy()),
    ));
    assert!(msg.contains("does not exist"), "{msg}");

    let file = dir.path().join("Cargo.lock");
    std::fs::write(&file, "x").unwrap();
    let msg = validation_msg(build_handoff_url(
        HandoffTarget::ClaudeCode,
        "hi",
        Some(&file.to_string_lossy()),
    ));
    assert!(msg.contains("does not exist"), "{msg}");
}

#[test]
fn state_of_reads_the_matching_target() {
    let t = AgentHandoffTargets {
        claude_code: HandoffState::NeedsFirstRun,
        claude_code_route: ClaudeCodeRoute::Prefill,
        vscode_claude: HandoffState::Ready,
        cursor: HandoffState::Missing,
    };
    assert_eq!(
        state_of(&t, HandoffTarget::ClaudeCode),
        HandoffState::NeedsFirstRun
    );
    assert_eq!(
        state_of(&t, HandoffTarget::VscodeClaude),
        HandoffState::Ready
    );
    assert_eq!(state_of(&t, HandoffTarget::Cursor), HandoffState::Missing);
}

#[test]
fn serde_names_match_the_frontend_contract() {
    assert_eq!(
        serde_json::to_string(&HandoffTarget::VscodeClaude).unwrap(),
        "\"vscode_claude\""
    );
    assert_eq!(
        serde_json::to_string(&HandoffState::NeedsFirstRun).unwrap(),
        "\"needs_first_run\""
    );
    let t: HandoffTarget = serde_json::from_str("\"claude_code\"").unwrap();
    assert_eq!(t, HandoffTarget::ClaudeCode);
    assert!(serde_json::from_str::<HandoffTarget>("\"javascript\"").is_err());
    assert_eq!(
        serde_json::to_string(&ClaudeCodeRoute::Paste).unwrap(),
        "\"paste\""
    );
    assert_eq!(
        serde_json::to_string(&HandoffOutcome::Prefilled).unwrap(),
        "\"prefilled\""
    );
}

// ---- Route decision: Windows Terminal present / absent, handler, binary ----

#[test]
fn windows_with_terminal_and_handler_uses_the_prefilled_link() {
    assert_eq!(
        claude_code_route(true, true, true, true),
        (HandoffState::Ready, ClaudeCodeRoute::Prefill)
    );
}

#[test]
fn windows_with_terminal_but_no_handler_asks_for_one_first_run() {
    assert_eq!(
        claude_code_route(true, true, false, true),
        (HandoffState::NeedsFirstRun, ClaudeCodeRoute::Prefill)
    );
}

#[test]
fn windows_without_terminal_pastes_even_when_the_handler_is_registered() {
    // The handler's PowerShell fallback exits at once there (Claude Code 2.1.319).
    assert_eq!(
        claude_code_route(true, false, true, true),
        (HandoffState::Ready, ClaudeCodeRoute::Paste)
    );
    assert_eq!(
        claude_code_route(true, false, false, true),
        (HandoffState::Ready, ClaudeCodeRoute::Paste)
    );
}

#[test]
fn a_missing_binary_means_no_claude_code_hand_off() {
    assert_eq!(
        claude_code_route(true, false, false, false).0,
        HandoffState::Missing
    );
    assert_eq!(
        claude_code_route(true, false, true, false).0,
        HandoffState::Missing
    );
    assert_eq!(
        claude_code_route(true, true, false, false).0,
        HandoffState::Missing
    );
    assert_eq!(
        claude_code_route(false, false, false, false).0,
        HandoffState::Missing
    );
}

#[test]
fn macos_and_linux_never_paste() {
    assert_eq!(
        claude_code_route(false, false, true, true),
        (HandoffState::Ready, ClaudeCodeRoute::Prefill)
    );
    assert_eq!(
        claude_code_route(false, false, false, true),
        (HandoffState::NeedsFirstRun, ClaudeCodeRoute::Prefill)
    );
}

// ---- Environment scrub ----

#[test]
fn claude_session_variables_are_recognised_in_any_case() {
    for name in [
        "CLAUDECODE",
        "claudecode",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_SSE_PORT",
        "CLAUDE_AGENT_SDK_VERSION",
        "Claude_Code_X",
    ] {
        assert!(is_claude_session_var(name), "{name}");
    }
    for name in [
        "PATH",
        "CLAUDE",
        "CLAUDE_CONFIG_DIR",
        "ANTHROPIC_API_KEY",
        "MY_CLAUDECODE",
    ] {
        assert!(!is_claude_session_var(name), "{name}");
    }
}

#[test]
fn scrub_removes_session_vars_but_keeps_the_users_persistent_config() {
    let current = [
        "PATH",
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_USE_BEDROCK",
        "HOME",
    ];
    let persistent = vec!["claude_code_use_bedrock".to_string(), "PATH".to_string()];
    assert_eq!(
        vars_to_scrub(current, &persistent),
        vec![
            "CLAUDECODE".to_string(),
            "CLAUDE_CODE_ENTRYPOINT".to_string()
        ]
    );
    assert!(vars_to_scrub(["PATH", "HOME"], &[]).is_empty());
}

// ---- The Windows paste route's console command ----

#[test]
fn console_start_args_open_claude_in_a_new_console_in_the_folder() {
    let args = console_start_args(
        Path::new(r"C:\Users\Dev User\.local\bin\claude.exe"),
        Path::new(r"D:\code\my app\"),
    )
    .unwrap();
    assert_eq!(
        args,
        r#"/c start "4DA" /D "D:\code\my app" "C:\Users\Dev User\.local\bin\claude.exe""#
    );
    // A drive root keeps its backslash.
    let root = console_start_args(Path::new(r"C:\claude.exe"), Path::new(r"C:\")).unwrap();
    assert!(root.contains(r#"/D "C:\""#), "{root}");
}

#[test]
fn console_start_args_refuse_what_cmd_would_interpret() {
    for (exe, dir) in [
        (r"C:\claude.exe", r"C:\100%done"),
        (r"C:\%TEMP%\claude.exe", r"C:\code"),
        (r#"C:\a"b\claude.exe"#, r"C:\code"),
        (r"C:\claude.exe", "C:\\code\u{202E}"),
    ] {
        assert!(
            console_start_args(Path::new(exe), Path::new(dir)).is_err(),
            "{exe} {dir}"
        );
    }
}

#[test]
fn handler_command_names_the_program() {
    if cfg!(windows) {
        assert_eq!(
            exe_from_handler_command(
                r#""C:\Users\Dev User\.local\bin\claude.exe" --handle-uri "%1""#
            ),
            Some(std::path::PathBuf::from(
                r"C:\Users\Dev User\.local\bin\claude.exe"
            ))
        );
        assert_eq!(
            exe_from_handler_command(r#"C:\tools\claude.exe --handle-uri "%1""#),
            Some(std::path::PathBuf::from(r"C:\tools\claude.exe"))
        );
    } else {
        assert_eq!(
            exe_from_handler_command(r#""/home/dev user/.local/bin/claude" --handle-uri "%1""#),
            Some(std::path::PathBuf::from("/home/dev user/.local/bin/claude"))
        );
    }
    assert_eq!(
        exe_from_handler_command(r#"claude.exe --handle-uri "%1""#),
        None
    );
    assert_eq!(exe_from_handler_command(""), None);
    assert_eq!(exe_from_handler_command(r#""""#), None);
}

#[test]
fn vscode_extension_is_found_by_its_folder_prefix() {
    let home = tempfile::tempdir().unwrap();
    assert!(!has_claude_vscode_extension(home.path()));
    let ext = home.path().join(".vscode").join("extensions");
    std::fs::create_dir_all(ext.join("ms-python.python-2026.1.0")).unwrap();
    assert!(!has_claude_vscode_extension(home.path()));
    std::fs::create_dir_all(ext.join("anthropic.claude-code-2.1.319")).unwrap();
    assert!(has_claude_vscode_extension(home.path()));
}

#[test]
fn mime_lists_name_a_scheme_handler_only_when_one_is_listed() {
    let cache = "[MIME Cache]\nx-scheme-handler/vscode=code-url-handler.desktop;\nx-scheme-handler/cursor=\n";
    assert!(mime_lists_scheme(cache, "vscode"));
    assert!(!mime_lists_scheme(cache, "cursor"));
    assert!(!mime_lists_scheme(cache, "claude-cli"));
    // A prefix of another scheme is not a match.
    assert!(!mime_lists_scheme(
        "x-scheme-handler/vscode-insiders=x.desktop;",
        "vscode"
    ));
}
