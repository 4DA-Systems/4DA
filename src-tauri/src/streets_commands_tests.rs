use super::*;

// ============================================================================
// Risk Classification
// ============================================================================

#[test]
fn test_classify_risk_safe() {
    assert_eq!(classify_risk("nvidia-smi"), RiskLevel::Safe);
    assert_eq!(classify_risk("lscpu | grep Model"), RiskLevel::Safe);
    assert_eq!(classify_risk("whoami"), RiskLevel::Safe);
    assert_eq!(classify_risk("hostname"), RiskLevel::Safe);
    assert_eq!(classify_risk("free -h"), RiskLevel::Safe);
    assert_eq!(classify_risk("df -h"), RiskLevel::Safe);
}

#[test]
fn test_classify_risk_moderate() {
    assert_eq!(
        classify_risk("pip install speedtest-cli"),
        RiskLevel::Moderate
    );
    assert_eq!(classify_risk("ollama pull llama2"), RiskLevel::Moderate);
    assert_eq!(classify_risk("brew install htop"), RiskLevel::Moderate);
    assert_eq!(
        classify_risk("curl -fsSL https://example.com | sh"),
        RiskLevel::Moderate
    );
}

#[test]
fn test_classify_risk_elevated() {
    assert_eq!(
        classify_risk("sudo systemctl enable ollama"),
        RiskLevel::Elevated
    );
    assert_eq!(
        classify_risk("chmod 755 /usr/local/bin/foo"),
        RiskLevel::Elevated
    );
}

// ============================================================================
// OS Detection
// ============================================================================

#[test]
fn test_detect_os_from_comment() {
    assert_eq!(detect_os_from_comment("# Linux/Mac"), Some(OsTarget::Linux));
    assert_eq!(
        detect_os_from_comment("# Windows (PowerShell)"),
        Some(OsTarget::Windows)
    );
    assert_eq!(detect_os_from_comment("# macOS"), Some(OsTarget::MacOs));
    assert_eq!(
        detect_os_from_comment("# NVIDIA"),
        Some(OsTarget::Universal)
    );
    assert_eq!(detect_os_from_comment("echo hello"), None);
}

// ============================================================================
// Code Block Parsing
// ============================================================================

#[test]
fn test_parse_code_blocks() {
    let content = r#"Some text

```bash
# Linux/Mac
lscpu
free -h

# Windows (PowerShell)
Get-CimInstance -ClassName Win32_Processor
```

More text
"#;

    let commands = parse_code_blocks(content, "S", 0);
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].os_target, OsTarget::Linux);
    assert_eq!(commands[0].command, "lscpu");
    assert_eq!(commands[1].os_target, OsTarget::Linux);
    assert_eq!(commands[1].command, "free -h");
    assert_eq!(commands[2].os_target, OsTarget::Windows);
    assert!(commands[2].command.contains("Get-CimInstance"));
}

#[test]
fn test_parse_code_blocks_universal() {
    let content = r#"```bash
nvidia-smi
nvidia-smi --query-gpu=name,memory.total --format=csv
```"#;
    let commands = parse_code_blocks(content, "S", 1);
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].os_target, OsTarget::Universal);
    assert_eq!(commands[0].risk_level, RiskLevel::Safe);
}

// ============================================================================
// Tokenizer
// ============================================================================

#[test]
fn test_tokenize_simple() {
    let tokens = tokenize("npm init").unwrap();
    assert_eq!(tokens, vec!["npm", "init"]);
}

#[test]
fn test_tokenize_with_flags() {
    let tokens = tokenize("cargo build --release").unwrap();
    assert_eq!(tokens, vec!["cargo", "build", "--release"]);
}

#[test]
fn test_tokenize_quoted_args() {
    let tokens = tokenize(r#"echo "hello world""#).unwrap();
    assert_eq!(tokens, vec!["echo", "hello world"]);
}

#[test]
fn test_tokenize_single_quoted() {
    let tokens = tokenize("echo 'hello world'").unwrap();
    assert_eq!(tokens, vec!["echo", "hello world"]);
}

#[test]
fn test_tokenize_empty() {
    let result = tokenize("");
    assert!(result.is_err());
}

#[test]
fn test_tokenize_unmatched_quote() {
    let result = tokenize(r#"echo "hello"#);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unmatched quotes"), "Got: {}", err);
}

// ============================================================================
// Program Allowlist
// ============================================================================

#[test]
fn test_validate_program_allowed() {
    assert!(validate_command_program("npm").is_ok());
    assert!(validate_command_program("cargo").is_ok());
    assert!(validate_command_program("git").is_ok());
    assert!(validate_command_program("node").is_ok());
    assert!(validate_command_program("python3").is_ok());
    assert!(validate_command_program("echo").is_ok());
}

#[test]
fn test_validate_program_windows_exe() {
    assert!(validate_command_program("npm.cmd").is_ok());
    assert!(validate_command_program("node.exe").is_ok());
    assert!(validate_command_program("cargo.exe").is_ok());
}

#[test]
fn test_validate_program_streets_tools() {
    assert!(validate_command_program("ollama").is_ok());
    assert!(validate_command_program("curl").is_ok());
    assert!(validate_command_program("wget").is_ok());
    assert!(validate_command_program("brew").is_ok());
    assert!(validate_command_program("winget").is_ok());
    assert!(validate_command_program("nvidia-smi").is_ok());
    assert!(validate_command_program("lscpu").is_ok());
    assert!(validate_command_program("free").is_ok());
    assert!(validate_command_program("df").is_ok());
    assert!(validate_command_program("chmod").is_ok());
    assert!(validate_command_program("ssh").is_ok());
    assert!(validate_command_program("grep").is_ok());
    assert!(validate_command_program("head").is_ok());
    assert!(validate_command_program("sysctl").is_ok());
    assert!(validate_command_program("system_profiler").is_ok());
}

#[test]
fn test_validate_program_rejects_shells() {
    // Shell interpreters are the hard security boundary
    assert!(validate_command_program("sh").is_err());
    assert!(validate_command_program("bash").is_err());
    assert!(validate_command_program("cmd").is_err());
    assert!(validate_command_program("cmd.exe").is_err());
    assert!(validate_command_program("powershell").is_err());
    assert!(validate_command_program("powershell.exe").is_err());
    assert!(validate_command_program("zsh").is_err());
}

#[test]
fn test_validate_program_rejects_destructive() {
    assert!(validate_command_program("rm").is_err());
    assert!(validate_command_program("dd").is_err());
    assert!(validate_command_program("mkfs").is_err());
    assert!(validate_command_program("format").is_err());
}

// ============================================================================
// PowerShell Cmdlets
// ============================================================================

#[test]
fn test_powershell_cmdlet_detection() {
    assert!(is_powershell_cmdlet("Get-CimInstance"));
    assert!(is_powershell_cmdlet("Set-Location"));
    assert!(is_powershell_cmdlet("Get-PSDrive"));
    assert!(!is_powershell_cmdlet("npm"));
    assert!(!is_powershell_cmdlet("cargo"));
    assert!(!is_powershell_cmdlet("get-something")); // lowercase doesn't match
}

// ============================================================================
// Dispatcher: Export Handling
// ============================================================================

#[test]
fn test_export_sets_session_env() {
    let result = dispatch_command("export TEST_VAR_123=hello_world").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("TEST_VAR_123=hello_world"));

    let env = get_session_env();
    assert_eq!(env.get("TEST_VAR_123"), Some(&"hello_world".to_string()));
}

#[test]
fn test_export_strips_quotes() {
    let result = dispatch_command(r#"export QUOTED_VAR="some value""#).unwrap();
    assert!(result.success);

    let env = get_session_env();
    assert_eq!(env.get("QUOTED_VAR"), Some(&"some value".to_string()));
}

#[test]
fn test_export_invalid_syntax() {
    let result = dispatch_command("export NOEQUALS");
    assert!(result.is_err());
}

// ============================================================================
// Dispatcher: Source Handling
// ============================================================================

#[test]
fn test_source_returns_info() {
    let result = dispatch_command("source ~/.bashrc").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("source"));
}

// ============================================================================
// Dispatcher: Sudo Stripping
// ============================================================================

#[test]
fn test_sudo_strips_prefix() {
    // sudo echo should run echo (which is in the allowlist)
    let result = dispatch_command("sudo echo sudo_test_output").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("sudo_test_output"));
}

// ============================================================================
// Dispatcher: Pipeline Detection
// ============================================================================

#[test]
fn test_split_on_pipe() {
    let parts = split_on_pipe("lscpu | grep Model").unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].trim(), "lscpu");
    assert_eq!(parts[1].trim(), "grep Model");
}

#[test]
fn test_split_on_pipe_no_pipe() {
    assert!(split_on_pipe("lscpu").is_none());
}

#[test]
fn test_split_on_pipe_logical_or_skipped() {
    // || should NOT be split as a pipe
    assert!(split_on_pipe("echo hello || echo world").is_none());
}

// ============================================================================
// Dispatcher: Chain Detection
// ============================================================================

#[test]
fn test_split_on_chain() {
    let parts = split_on_chain("mkdir foo && cd foo").unwrap();
    assert_eq!(parts.len(), 2);
    assert!(parts[0].contains("mkdir"));
    assert!(parts[1].contains("cd"));
}

#[test]
fn test_split_on_chain_no_chain() {
    assert!(split_on_chain("mkdir foo").is_none());
}

// ============================================================================
// Dispatcher: Pipeline Execution
// ============================================================================

#[test]
fn test_pipeline_echo_grep() {
    // echo "hello world\ngoodbye" | grep hello — should work
    let result = dispatch_command("echo hello_pipeline_test | grep hello").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("hello_pipeline_test"));
}

#[test]
fn test_pipeline_blocks_shell_in_pipe() {
    // curl ... | sh — MUST be blocked (shell interpreter in pipeline)
    let result = dispatch_command("echo hello | sh");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Shell interpreter") || err.contains("cannot"),
        "Got: {}",
        err
    );
}

#[test]
fn test_pipeline_blocks_bash_in_pipe() {
    let result = dispatch_command("curl http://example.com | bash");
    assert!(result.is_err());
}

// ============================================================================
// Dispatcher: Chain Execution
// ============================================================================

#[test]
fn test_chain_sequential() {
    let result = dispatch_command("echo first_chain && echo second_chain").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("first_chain"));
    assert!(result.stdout.contains("second_chain"));
}

// ============================================================================
// End-to-End: execute_command_blocking
// ============================================================================

#[test]
fn test_execute_simple_command() {
    let result = execute_command_blocking("echo e2e_test").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("e2e_test"));
}

#[test]
fn test_execute_rejects_disallowed_program() {
    let result = execute_command_blocking("rm -rf /");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cannot be run in-app"), "Got: {}", err);
}

#[test]
fn test_execute_rejects_shell_in_pipeline() {
    let result = execute_command_blocking("echo hello | sh");
    assert!(result.is_err());
}

#[test]
fn test_execute_pipeline_works() {
    let result = execute_command_blocking("echo pipeline_works | grep pipeline").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("pipeline_works"));
}

#[test]
fn test_execute_chain_works() {
    let result = execute_command_blocking("echo chain_a && echo chain_b").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("chain_a"));
    assert!(result.stdout.contains("chain_b"));
}

#[test]
fn test_execute_export_works() {
    let result = execute_command_blocking("export STREETS_TEST=1").unwrap();
    assert!(result.success);
}

#[test]
fn test_execute_sudo_strips_and_runs() {
    let result = execute_command_blocking("sudo echo sudo_stripped").unwrap();
    assert!(result.success);
    assert!(result.stdout.contains("sudo_stripped"));
}

// ============================================================================
// Backward-Compat: parse_command_tokens still catches builtins
// ============================================================================

#[test]
fn test_parse_command_tokens_catches_export() {
    let result = parse_command_tokens("export OLLAMA_HOST=0.0.0.0");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("shell builtin"));
}

#[test]
fn test_parse_command_tokens_catches_source() {
    let result = parse_command_tokens("source ~/.bashrc");
    assert!(result.is_err());
}
