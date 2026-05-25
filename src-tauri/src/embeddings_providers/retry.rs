// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Retry logic with exponential backoff for embedding operations.

use crate::error::Result;

/// Retry an async operation with exponential backoff.
/// Returns the first successful result, or the last error after max_retries.
/// Rate-limit errors (containing "rate limit" or "429") use an extended backoff
/// of 30s instead of the normal exponential schedule.
pub(in crate::embeddings) async fn retry_with_backoff<F, Fut, T>(
    operation_name: &str,
    max_retries: u32,
    f: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_error = String::new();
    for attempt in 0..=max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = e.to_string();
                if attempt < max_retries {
                    // Detect rate-limit errors and use extended backoff
                    let lower = last_error.to_lowercase();
                    let is_rate_limited = lower.contains("rate limit")
                        || lower.contains("429")
                        || lower.contains("too many requests");

                    let delay_secs = if is_rate_limited {
                        // Parse retry-after hint from error message if present
                        let retry_after = lower
                            .find("retry after ")
                            .and_then(|pos| {
                                let after = &last_error[pos + 12..];
                                after
                                    .chars()
                                    .take_while(|c| c.is_ascii_digit())
                                    .collect::<String>()
                                    .parse::<u64>()
                                    .ok()
                            })
                            .unwrap_or(30);
                        tracing::warn!(
                            target: "4da::retry",
                            attempt = attempt + 1,
                            max = max_retries + 1,
                            delay_secs = retry_after,
                            operation = operation_name,
                            "Rate limited — using extended backoff"
                        );
                        retry_after
                    } else {
                        let delay = 3u64.pow(attempt); // 1s, 3s, 9s
                        tracing::warn!(
                            target: "4da::retry",
                            attempt = attempt + 1,
                            max = max_retries + 1,
                            delay_secs = delay,
                            operation = operation_name,
                            error = %last_error,
                            "Retrying after error"
                        );
                        delay
                    };
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                }
            }
        }
    }
    Err(last_error.into())
}
