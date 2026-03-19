mod heartbeat;

#[cfg(feature = "void-universe")]
#[allow(dead_code)] // Feature-gated: only active when "void-universe" is enabled
mod universe;

pub use heartbeat::*;

#[cfg(feature = "void-universe")]
#[allow(unused_imports)]
pub use universe::*;
