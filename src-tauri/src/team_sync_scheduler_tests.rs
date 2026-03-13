use super::*;

#[test]
fn default_state_is_disabled_and_unconfigured() {
    let state = TeamSyncState::default();
    assert!(!state.enabled.load(Ordering::Relaxed));
    assert!(!state.connected.load(Ordering::Relaxed));
    assert_eq!(state.last_sync.load(Ordering::Relaxed), 0);
    assert_eq!(state.sync_interval_secs.load(Ordering::Relaxed), 30);
    assert!(!state.is_configured());
}

#[test]
fn configure_from_relay_config() {
    let state = TeamSyncState::default();
    let config = TeamRelayConfig {
        enabled: true,
        relay_url: Some("https://relay.4da.ai".to_string()),
        auth_token: Some("tok_test".to_string()),
        team_id: Some("team-123".to_string()),
        client_id: Some("client-456".to_string()),
        display_name: Some("Alice".to_string()),
        role: Some("admin".to_string()),
        sync_interval_secs: Some(60),
    };
    state.configure(&config);

    assert!(state.enabled.load(Ordering::Relaxed));
    assert_eq!(state.sync_interval_secs.load(Ordering::Relaxed), 60);
    assert_eq!(state.team_id.lock().as_deref(), Some("team-123"));
    assert_eq!(
        state.relay_url.lock().as_deref(),
        Some("https://relay.4da.ai")
    );

    // Still not configured because team_key is not set
    assert!(!state.is_configured());

    // Set team key
    *state.team_key.lock() = Some([42u8; 32]);
    assert!(state.is_configured());
}

#[test]
fn is_configured_requires_all_fields() {
    let state = TeamSyncState::default();

    // Set all except team_key -- should be false
    *state.team_id.lock() = Some("t".to_string());
    *state.client_id.lock() = Some("c".to_string());
    *state.relay_url.lock() = Some("u".to_string());
    *state.auth_token.lock() = Some("a".to_string());
    assert!(!state.is_configured());

    // Set team_key -- should be true
    *state.team_key.lock() = Some([0u8; 32]);
    assert!(state.is_configured());
}

#[test]
fn configure_uses_default_interval_when_none() {
    let state = TeamSyncState::default();
    let config = TeamRelayConfig {
        enabled: true,
        sync_interval_secs: None,
        ..Default::default()
    };
    state.configure(&config);
    assert_eq!(state.sync_interval_secs.load(Ordering::Relaxed), 30);
}

#[test]
fn configure_can_disable() {
    let state = TeamSyncState::default();
    state.enabled.store(true, Ordering::Relaxed);

    let config = TeamRelayConfig {
        enabled: false,
        ..Default::default()
    };
    state.configure(&config);
    assert!(!state.enabled.load(Ordering::Relaxed));
}
