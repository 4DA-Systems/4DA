mod threshold;
mod score_component;
mod confirmation_gate;
mod scoring_builder;

use proc_macro::TokenStream;

/// Validates that a `const` float value falls within a compile-time range.
///
/// # Usage
/// ```rust
/// #[threshold(0.20, 0.80)]
/// const CONTEXT_THRESHOLD: f32 = 0.45;
/// ```
///
/// Generates a companion const assertion that triggers a compile error
/// if the value is outside `[min, max]`.
#[proc_macro_attribute]
pub fn threshold(attr: TokenStream, item: TokenStream) -> TokenStream {
    threshold::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Wraps a scoring function's return value with `debug_assert!` bounds checking.
///
/// # Usage
/// ```rust
/// #[score_component(output_range = "0.0..=1.0")]
/// pub(crate) fn compute_affinity(ctx: &Ctx) -> f32 {
///     // ...
/// }
/// ```
///
/// In debug builds, asserts the return value is not NaN and falls within
/// the specified inclusive range.
#[proc_macro_attribute]
pub fn score_component(attr: TokenStream, item: TokenStream) -> TokenStream {
    score_component::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Validates struct fields against declared signal axes and generates helpers.
///
/// # Usage
/// ```rust
/// #[confirmation_gate(axes = ["context", "interest", "ace", "learned"])]
/// struct SignalConfirmation {
///     context_confirmed: bool,
///     interest_confirmed: bool,
///     ace_confirmed: bool,
///     learned_confirmed: bool,
///     count: u8,
/// }
/// ```
///
/// For each axis `"foo"`, validates that a field `foo_confirmed: bool` exists.
/// Generates `AXIS_COUNT` and `AXIS_NAMES` associated constants.
#[proc_macro_attribute]
pub fn confirmation_gate(attr: TokenStream, item: TokenStream) -> TokenStream {
    confirmation_gate::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Derives a type-safe builder with sensible defaults for test construction.
///
/// # Usage
/// ```rust
/// #[derive(ScoringBuilder)]
/// struct ScoringContext {
///     cached_context_count: i64,
///     topics: Vec<String>,
///     name: String,
///     active: bool,
/// }
/// ```
///
/// Generates `ScoringContext::builder()` returning a `ScoringContextBuilder`
/// where every field has a default (0 for numbers, empty for collections, etc.)
/// and a fluent setter. Call `.build()` to produce the final struct.
#[proc_macro_derive(ScoringBuilder)]
pub fn derive_scoring_builder(item: TokenStream) -> TokenStream {
    scoring_builder::expand(item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
