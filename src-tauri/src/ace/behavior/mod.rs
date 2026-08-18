// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Behavior tracking — explicit user interaction recording (v20b, AD-031:
//! the implicit topic-affinity/anti-topic learning layer was removed).

mod decay;
pub(crate) mod tracking;
mod types;

pub use types::*;
