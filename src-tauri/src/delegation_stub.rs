// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Stub for `delegation` module when "experimental" feature is disabled.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DelegationScore {
    pub subject: String,
    pub overall_score: f64,
    pub factors: DelegationFactors,
    pub recommendation: DelegationRec,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "bindings/")]
pub struct DelegationFactors {
    pub pattern_stability: f64,
    pub security_sensitivity: f64,
    pub codebase_complexity: f64,
    pub decision_density: f64,
    pub ai_track_record: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "bindings/")]
pub enum DelegationRec {
    FullyDelegate,
    DelegateWithReview,
    CollaborateRealtime,
    HumanOnly,
}

#[tauri::command]
pub async fn get_delegation_score(_subject: String) -> Result<DelegationScore> {
    Err("Delegation scoring is an experimental feature".into())
}

#[tauri::command]
pub async fn get_all_delegation_scores() -> Result<Vec<DelegationScore>> {
    Err("Delegation scoring is an experimental feature".into())
}
