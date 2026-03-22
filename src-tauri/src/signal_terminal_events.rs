// Copyright (c) 2025-2026 4DA Systems Pty Ltd. All rights reserved.
// Licensed under FSL-1.1-Apache-2.0.

//! Event broadcaster for Signal Terminal SSE streaming.
//! Bridges Tauri app events to HTTP SSE clients.

use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Events sent to terminal SSE clients.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum TerminalEvent {
    /// System status update (monitoring, counts, etc.)
    Status {
        monitoring: bool,
        signals_count: usize,
        total_scanned: usize,
    },
    /// Analysis started
    AnalysisStarted,
    /// Analysis progress
    AnalysisProgress {
        stage: String,
        progress: f32,
        message: String,
    },
    /// Analysis completed with result summary
    AnalysisComplete {
        relevant_count: usize,
        total_count: usize,
    },
    /// Heartbeat (keepalive with minimal data)
    Heartbeat {
        pulse: f32,
        critical_count: u32,
    },
}

static BROADCASTER: Lazy<Arc<broadcast::Sender<TerminalEvent>>> = Lazy::new(|| {
    let (tx, _) = broadcast::channel(128);
    Arc::new(tx)
});

/// Get a sender to broadcast events.
pub fn broadcast(event: TerminalEvent) {
    let _ = BROADCASTER.send(event);
}

/// Subscribe to the event stream. Returns a receiver.
pub fn subscribe() -> broadcast::Receiver<TerminalEvent> {
    BROADCASTER.subscribe()
}
