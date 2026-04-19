// SPDX-License-Identifier: FSL-1.1-Apache-2.0
mod heartbeat;

#[cfg(feature = "void-universe")]
#[allow(dead_code)] // Feature-gated: only active when "void-universe" is enabled
mod universe;

pub use heartbeat::*;

#[cfg(feature = "void-universe")]
#[allow(unused_imports)]
pub use universe::*;
