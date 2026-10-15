// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Email delivery for 4DA digest system.
//!
//! Privacy-first design: emails go directly from user's machine to their SMTP
//! provider. 4DA never sees the email address or digest contents. Off by default.

use lettre::message::{header::ContentType, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tracing::info;

use crate::digest::{Digest, SmtpConfig};
use crate::error::Result;

/// Send a compiled digest via the user's SMTP configuration.
///
/// Returns Ok(()) on successful send, or an error describing the failure.
/// The caller should NOT retry — the next scheduled cycle will try again.
pub async fn send_digest_email(to_address: &str, smtp: &SmtpConfig, digest: &Digest) -> Result<()> {
    let html_body = digest.to_html();
    let text_body = digest.to_text();

    let subject = format_subject(digest);

    let email = Message::builder()
        .from(smtp.from_address.parse().map_err(|e| {
            crate::error::FourDaError::Config(format!(
                "Invalid from address '{}': {e}",
                smtp.from_address
            ))
        })?)
        .to(to_address.parse().map_err(|e| {
            crate::error::FourDaError::Config(format!(
                "Invalid recipient address '{to_address}': {e}"
            ))
        })?)
        .subject(&subject)
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(text_body),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html_body),
                ),
        )
        .map_err(|e| crate::error::FourDaError::Config(format!("Failed to build email: {e}")))?;

    let transport = build_transport(smtp)?;

    transport
        .send(email)
        .await
        .map_err(|e| crate::error::FourDaError::Config(format!("SMTP send failed: {e}")))?;

    info!(target: "4da::email", to = %to_address, "Digest email sent successfully");
    Ok(())
}

/// Send a test email to verify SMTP configuration.
///
/// Sends a minimal email with no digest content — just confirms delivery works.
pub async fn send_test_email(to_address: &str, smtp: &SmtpConfig) -> Result<()> {
    let email = Message::builder()
        .from(
            smtp.from_address
                .parse()
                .map_err(|e| crate::error::FourDaError::Config(format!(
                    "Invalid from address '{}': {e}", smtp.from_address
                )))?,
        )
        .to(
            to_address
                .parse()
                .map_err(|e| crate::error::FourDaError::Config(format!(
                    "Invalid recipient address '{to_address}': {e}"
                )))?,
        )
        .subject("4DA — Test Email")
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body("This is a test email from 4DA. Your digest email configuration is working correctly.".to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(test_email_html()),
                ),
        )
        .map_err(|e| crate::error::FourDaError::Config(format!("Failed to build test email: {e}")))?;

    let transport = build_transport(smtp)?;

    transport
        .send(email)
        .await
        .map_err(|e| crate::error::FourDaError::Config(format!("SMTP test failed: {e}")))?;

    info!(target: "4da::email", to = %to_address, "Test email sent successfully");
    Ok(())
}

/// Build an async SMTP transport from user config.
fn build_transport(smtp: &SmtpConfig) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    // settings.json is hand-editable; never send credentials in the clear to a
    // remote server whatever it says.
    plaintext_smtp_allowed(&smtp.host, smtp.use_tls).map_err(crate::error::FourDaError::Config)?;
    let creds = Credentials::new(smtp.username.clone(), smtp.password.clone());

    let builder = if smtp.use_tls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host).map_err(|e| {
            crate::error::FourDaError::Config(format!(
                "SMTP TLS relay setup failed for '{}': {e}",
                smtp.host
            ))
        })?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host)
    };

    Ok(builder
        .port(smtp.port)
        .credentials(creds)
        .timeout(Some(std::time::Duration::from_secs(30)))
        .build())
}

/// Format the email subject line based on digest frequency and content.
fn format_subject(digest: &Digest) -> String {
    let date = digest.created_at.format("%b %d, %Y");
    let duration = digest.period_end - digest.period_start;

    if duration.num_days() >= 6 {
        format!("4DA Weekly Digest — Week of {date}")
    } else {
        let count = digest.summary.total_items;
        let signals = digest.summary.critical_count + digest.summary.high_count;
        if signals > 0 {
            format!("4DA Digest — {count} items, {signals} signals ({date})")
        } else {
            format!("4DA Digest — {count} items ({date})")
        }
    }
}

/// HTML body for the test email — minimal, matches 4DA design.
fn test_email_html() -> String {
    r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 500px; margin: 0 auto; padding: 40px 20px; background: #0A0A0A; color: #FFFFFF;">
    <div style="text-align: center; padding: 30px 0; border-bottom: 1px solid #2A2A2A;">
        <h1 style="margin: 0; font-size: 20px; color: #D4AF37;">4DA</h1>
    </div>
    <div style="padding: 30px 0; text-align: center;">
        <div style="background: #141414; padding: 20px; border-radius: 8px; border: 1px solid #2A2A2A;">
            <p style="margin: 0 0 8px; font-size: 16px;">Email delivery is working.</p>
            <p style="margin: 0; color: #A0A0A0; font-size: 14px;">Your digest will arrive at the configured frequency.</p>
        </div>
    </div>
    <div style="text-align: center; padding: 20px 0; color: #666666; font-size: 12px; border-top: 1px solid #2A2A2A;">
        <a href="https://4da.ai" style="color: #D4AF37; text-decoration: none;">4DA</a> — All signal. No feed.
    </div>
</body>
</html>"#.to_string()
}

/// Fields a `set_digest_email_config` call may change; `None` = unchanged.
pub(crate) struct SmtpUpdate {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_address: Option<String>,
    pub use_tls: Option<bool>,
}

impl SmtpUpdate {
    fn is_empty(&self) -> bool {
        self.host.is_none()
            && self.port.is_none()
            && self.username.is_none()
            && self.password.is_none()
            && self.from_address.is_none()
            && self.use_tls.is_none()
    }
}

fn normalized_smtp_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_lowercase()
}

/// Apply an update to the stored SMTP config (audit C6).
///
/// * The password stays bound to the server it was entered for: a host change
///   that does not carry a new password clears it, so one IPC call cannot
///   redirect the saved credential to another mail server.
/// * Plaintext SMTP (`use_tls = false`) is refused unless the server is this
///   machine — otherwise the password crosses the network in the clear.
pub(crate) fn merge_smtp_update(
    current: Option<SmtpConfig>,
    update: SmtpUpdate,
) -> std::result::Result<SmtpConfig, String> {
    let current = current.unwrap_or(SmtpConfig {
        host: String::new(),
        port: 587,
        username: String::new(),
        password: String::new(),
        from_address: String::new(),
        use_tls: true,
    });
    let host = update.host.unwrap_or_else(|| current.host.clone());
    let host_changed = normalized_smtp_host(&host) != normalized_smtp_host(&current.host);
    let password = match update.password {
        Some(p) => p,
        None if host_changed => String::new(),
        None => current.password,
    };
    let use_tls = update.use_tls.unwrap_or(current.use_tls);
    plaintext_smtp_allowed(&host, use_tls)?;
    Ok(SmtpConfig {
        host,
        port: update.port.unwrap_or(current.port),
        username: update.username.unwrap_or(current.username),
        password,
        from_address: update.from_address.unwrap_or(current.from_address),
        use_tls,
    })
}

/// `use_tls = false` is only acceptable when the SMTP server is loopback (a
/// local relay or test server); anywhere else the credentials would be sent
/// unencrypted.
fn plaintext_smtp_allowed(host: &str, use_tls: bool) -> std::result::Result<(), String> {
    if use_tls || host.trim().is_empty() || crate::url_validation::is_loopback_host(host) {
        Ok(())
    } else {
        Err(format!(
            "Refusing unencrypted SMTP to {}: TLS is required for any server other than this machine",
            host.trim()
        ))
    }
}

// ==========================================================================
// Tauri Commands
// ==========================================================================

/// Test digest email delivery — sends a test email via user's SMTP config.
#[tauri::command]
pub async fn test_digest_email() -> std::result::Result<String, String> {
    let (email, smtp) = {
        let settings = crate::get_settings_manager().lock();
        let digest = &settings.get().digest;
        (digest.email.clone(), digest.smtp.clone())
    };

    let email = email.ok_or("No email address configured. Set your email in Digest settings.")?;
    let smtp =
        smtp.ok_or("No SMTP configuration. Configure your email provider in Digest settings.")?;

    send_test_email(&email, &smtp)
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!("Test email sent to {email}"))
}

/// Save SMTP configuration for digest email delivery.
/// Password is held in memory only — not written to settings.json (skip_serializing).
#[tauri::command]
pub async fn set_digest_email_config(
    email: Option<String>,
    smtp_host: Option<String>,
    smtp_port: Option<u16>,
    smtp_username: Option<String>,
    smtp_password: Option<String>,
    smtp_from: Option<String>,
    smtp_use_tls: Option<bool>,
) -> std::result::Result<String, String> {
    let mut settings = crate::get_settings_manager().lock();
    let digest = &mut settings.get_mut().digest;

    // Update email address
    if let Some(addr) = email {
        digest.email = if addr.is_empty() { None } else { Some(addr) };
    }

    // Update SMTP config — create if any field provided, update if exists
    let update = SmtpUpdate {
        host: smtp_host,
        port: smtp_port,
        username: smtp_username,
        password: smtp_password,
        from_address: smtp_from,
        use_tls: smtp_use_tls,
    };
    if !update.is_empty() {
        digest.smtp = Some(merge_smtp_update(digest.smtp.clone(), update)?);
    }

    settings.save().map_err(|e| e.to_string())?;
    Ok("Email configuration saved".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::{Digest, DigestItem, SmtpConfig};
    use chrono::{Duration, Utc};

    #[test]
    fn test_format_subject_daily() {
        let items = vec![DigestItem {
            id: 1,
            title: "Test".to_string(),
            url: None,
            source: "hn".to_string(),
            relevance_score: 0.8,
            matched_topics: vec![],
            discovered_at: Utc::now(),
            summary: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
        }];
        let digest = Digest::new(items, Utc::now() - Duration::hours(24), Utc::now());
        let subject = format_subject(&digest);
        assert!(subject.starts_with("4DA Digest"));
        assert!(subject.contains("1 items"));
    }

    #[test]
    fn test_format_subject_weekly() {
        let items = vec![DigestItem {
            id: 1,
            title: "Test".to_string(),
            url: None,
            source: "hn".to_string(),
            relevance_score: 0.8,
            matched_topics: vec![],
            discovered_at: Utc::now(),
            summary: None,
            signal_type: None,
            signal_priority: None,
            signal_action: None,
        }];
        let digest = Digest::new(items, Utc::now() - Duration::days(7), Utc::now());
        let subject = format_subject(&digest);
        assert!(subject.starts_with("4DA Weekly Digest"));
    }

    #[test]
    fn test_format_subject_with_signals() {
        let items = vec![DigestItem {
            id: 1,
            title: "CVE Found".to_string(),
            url: None,
            source: "hn".to_string(),
            relevance_score: 0.9,
            matched_topics: vec!["security".to_string()],
            discovered_at: Utc::now(),
            summary: None,
            signal_type: Some("security_alert".to_string()),
            signal_priority: Some("critical".to_string()),
            signal_action: Some("Review CVE".to_string()),
        }];
        let digest = Digest::new(items, Utc::now() - Duration::hours(24), Utc::now());
        let subject = format_subject(&digest);
        assert!(subject.contains("1 signals"));
    }

    #[test]
    fn test_build_transport_tls() {
        let smtp = SmtpConfig {
            host: "smtp.example.com".to_string(),
            port: 587,
            username: "user".to_string(),
            password: "pass".to_string(),
            from_address: "test@example.com".to_string(),
            use_tls: true,
        };
        let result = build_transport(&smtp);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_transport_no_tls() {
        let smtp = SmtpConfig {
            host: "localhost".to_string(),
            port: 25,
            username: "user".to_string(),
            password: "pass".to_string(),
            from_address: "test@localhost".to_string(),
            use_tls: false,
        };
        let result = build_transport(&smtp);
        assert!(result.is_ok());
    }

    #[test]
    fn test_test_email_html_renders() {
        let html = test_email_html();
        assert!(html.contains("4DA"));
        assert!(html.contains("Email delivery is working"));
        assert!(html.contains("#0A0A0A"));
    }

    fn stored_smtp() -> SmtpConfig {
        SmtpConfig {
            host: "smtp.fastmail.com".into(),
            port: 587,
            username: "me@example.com".into(),
            password: "fake-app-password".into(),
            from_address: "me@example.com".into(),
            use_tls: true,
        }
    }

    fn update() -> SmtpUpdate {
        SmtpUpdate {
            host: None,
            port: None,
            username: None,
            password: None,
            from_address: None,
            use_tls: None,
        }
    }

    /// C6: changing only the host must not carry the saved password to it.
    #[test]
    fn host_change_without_password_clears_it() {
        let merged = merge_smtp_update(
            Some(stored_smtp()),
            SmtpUpdate {
                host: Some("smtp.attacker.example".into()),
                ..update()
            },
        )
        .expect("merge");
        assert_eq!(merged.host, "smtp.attacker.example");
        assert!(
            merged.password.is_empty(),
            "password must not follow a host change"
        );
    }

    #[test]
    fn host_change_with_new_password_keeps_the_new_one() {
        let merged = merge_smtp_update(
            Some(stored_smtp()),
            SmtpUpdate {
                host: Some("smtp.gmail.com".into()),
                password: Some("new-secret".into()),
                ..update()
            },
        )
        .expect("merge");
        assert_eq!(merged.password, "new-secret");
    }

    #[test]
    fn same_host_edits_keep_the_password() {
        let merged = merge_smtp_update(
            Some(stored_smtp()),
            SmtpUpdate {
                host: Some(" SMTP.Fastmail.com. ".into()),
                port: Some(465),
                ..update()
            },
        )
        .expect("merge");
        assert_eq!(merged.password, "fake-app-password");
        assert_eq!(merged.port, 465);
    }

    #[test]
    fn plaintext_smtp_is_refused_for_remote_hosts() {
        let err = merge_smtp_update(
            Some(stored_smtp()),
            SmtpUpdate {
                use_tls: Some(false),
                ..update()
            },
        )
        .expect_err("remote plaintext must be refused");
        assert!(err.contains("TLS is required"), "{err}");
    }

    #[test]
    fn plaintext_smtp_is_allowed_for_loopback() {
        let merged = merge_smtp_update(
            None,
            SmtpUpdate {
                host: Some("127.0.0.1".into()),
                port: Some(1025),
                use_tls: Some(false),
                ..update()
            },
        )
        .expect("loopback plaintext is fine");
        assert!(!merged.use_tls);
    }

    /// A hand-edited settings.json with `use_tls: false` is refused at send time.
    #[tokio::test]
    async fn transport_refuses_remote_plaintext() {
        let mut smtp = stored_smtp();
        smtp.use_tls = false;
        assert!(build_transport(&smtp).is_err());
        smtp.host = "localhost".into();
        assert!(build_transport(&smtp).is_ok());
    }
}
