// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Simple IPC rate limiter for expensive Tauri commands.
//!
//! Prevents runaway frontend code from hammering resource-intensive
//! backend operations (LLM calls, full analyses, batch operations).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Global rate limiter for expensive IPC commands.
static RATE_LIMITER: std::sync::LazyLock<Mutex<RateLimiter>> =
    std::sync::LazyLock::new(|| Mutex::new(RateLimiter::new()));

struct RateLimiter {
    /// command_name -> (last_call_time, call_count_in_window)
    windows: HashMap<String, (Instant, u32)>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    /// Check if a command is within its rate limit.
    /// Returns Ok(()) if allowed, Err with message if rate-limited.
    fn check(&mut self, command: &str, max_per_minute: u32) -> Result<(), String> {
        let now = Instant::now();
        let entry = self.windows.entry(command.to_string()).or_insert((now, 0));

        // Reset window if more than 60 seconds have passed
        if now.duration_since(entry.0).as_secs() >= 60 {
            *entry = (now, 1);
            return Ok(());
        }

        entry.1 += 1;
        if entry.1 > max_per_minute {
            tracing::warn!(
                target: "4da::ipc",
                command,
                count = entry.1,
                limit = max_per_minute,
                "IPC rate limit exceeded"
            );
            return Err(format!(
                "Rate limit exceeded for {command}: max {max_per_minute} calls per minute"
            ));
        }

        Ok(())
    }
}

/// Check rate limit for an expensive IPC command.
/// Call at the start of resource-intensive Tauri commands.
pub(crate) fn check_rate_limit(command: &str, max_per_minute: u32) -> crate::error::Result<()> {
    match RATE_LIMITER.lock() {
        Ok(mut limiter) => limiter
            .check(command, max_per_minute)
            .map_err(crate::error::FourDaError::Validation),
        Err(_) => Ok(()), // Poisoned mutex — allow through rather than block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_under_limit() {
        let mut limiter = RateLimiter::new();
        for _ in 0..5 {
            assert!(limiter.check("test_cmd", 10).is_ok());
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let mut limiter = RateLimiter::new();
        for _ in 0..10 {
            let _ = limiter.check("test_cmd", 10);
        }
        // 11th call should be blocked
        assert!(limiter.check("test_cmd", 10).is_err());
    }

    #[test]
    fn test_rate_limiter_independent_commands() {
        let mut limiter = RateLimiter::new();
        for _ in 0..10 {
            let _ = limiter.check("cmd_a", 10);
        }
        // cmd_b should still be allowed
        assert!(limiter.check("cmd_b", 10).is_ok());
    }
}
