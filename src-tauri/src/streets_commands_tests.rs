use super::*;

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
