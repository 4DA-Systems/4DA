// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Curated Seed Corpus — Intelligence Reconciliation Phase 8 (Cold Start Layer 2).
//!
//! The curated corpus is compiled into the binary via `include_str!` on
//! `src-tauri/src/seed_data/decisions.jsonl`. The corpus is available
//! implicitly through the git decision miner's `SeededDecision` type.
//!
//! Previous Tauri command surface (`get_seed_corpus_stats`) and associated
//! helpers were removed as phantom commands (deregistered from invoke_handler
//! but code left behind).
