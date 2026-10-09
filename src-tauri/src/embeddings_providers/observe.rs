// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Report whether an embed call had to load the model (a cold embedder).
//!
//! Ollama returns `load_duration` (nanoseconds) on every `/api/embed` reply: near
//! zero when the model was resident, the whole load when it was not. A caller that
//! wants to know runs its embed inside [`observe_embed_load`]; the provider records
//! into the task-local only when one is in scope, so background embeds pay nothing.

use std::cell::Cell;
use std::future::Future;

tokio::task_local! {
    static EMBED_LOAD_NS: Cell<Option<u64>>;
}

/// Record Ollama's `load_duration` for the observing caller, if there is one.
pub(in crate::embeddings) fn record_load_duration(json: &serde_json::Value) {
    if let Some(ns) = json
        .get("load_duration")
        .and_then(serde_json::Value::as_u64)
    {
        let _ = EMBED_LOAD_NS.try_with(|c| c.set(Some(ns)));
    }
}

/// Run `fut` and return its output with the model load time Ollama reported, in
/// milliseconds. `None` means no Ollama embed ran inside it (another provider, or
/// the embed failed before a reply).
pub(crate) async fn observe_embed_load<F: Future>(fut: F) -> (F::Output, Option<u64>) {
    EMBED_LOAD_NS
        .scope(Cell::new(None), async move {
            let out = fut.await;
            let load_ms = EMBED_LOAD_NS.with(Cell::get).map(|ns| ns / 1_000_000);
            (out, load_ms)
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reports_the_load_duration_inside_the_scope() {
        let reply = serde_json::json!({ "embeddings": [], "load_duration": 2_345_000_000u64 });
        let ((), load_ms) = observe_embed_load(async { record_load_duration(&reply) }).await;
        assert_eq!(load_ms, Some(2345));
    }

    #[tokio::test]
    async fn no_reply_means_no_load_reported() {
        let ((), load_ms) = observe_embed_load(async {}).await;
        assert_eq!(load_ms, None);
    }

    #[test]
    fn recording_outside_a_scope_is_a_no_op() {
        // A background embed has no observer; it must not panic.
        record_load_duration(&serde_json::json!({ "load_duration": 1u64 }));
    }
}
