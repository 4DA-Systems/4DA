// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Minimal stub -- only TeamRelayConfig is needed by settings deserialization.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamRelayConfig {
    pub enabled: bool,
    pub relay_url: Option<String>,
    pub auth_token: Option<String>,
    pub team_id: Option<String>,
    pub client_id: Option<String>,
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub sync_interval_secs: Option<u64>,
}
