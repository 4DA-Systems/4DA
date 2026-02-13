use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, ItemFn, LitStr, MetaNameValue};

/// Expand `#[score_component(output_range = "0.0..=1.0")]` on a function.
///
/// The original function body is wrapped in an immediately-invoked closure
/// and the return value is checked with `debug_assert!` for NaN and range.
pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    // --- parse attribute: output_range = "min..=max" -----------------------
    let meta: MetaNameValue = parse2(attr)?;

    let path_str = meta
        .path
        .get_ident()
        .map(|id| id.to_string())
        .unwrap_or_default();
    if path_str != "output_range" {
        return Err(syn::Error::new_spanned(
            &meta.path,
            "score_component: expected `output_range = \"min..=max\"`",
        ));
    }

    let range_lit: LitStr = match &meta.value {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => s.clone(),
        _ => {
            return Err(syn::Error::new_spanned(
                &meta.value,
                "score_component: output_range value must be a string literal like \"0.0..=1.0\"",
            ));
        }
    };

    let range_str = range_lit.value();
    let (min_str, max_str) = range_str.split_once("..=").ok_or_else(|| {
        syn::Error::new_spanned(
            &range_lit,
            "score_component: output_range must use inclusive range syntax \"min..=max\"",
        )
    })?;

    let min_val: f64 = min_str.trim().parse::<f64>().map_err(|_| {
        syn::Error::new_spanned(
            &range_lit,
            format!("score_component: cannot parse min bound `{min_str}` as a float"),
        )
    })?;
    let max_val: f64 = max_str.trim().parse::<f64>().map_err(|_| {
        syn::Error::new_spanned(
            &range_lit,
            format!("score_component: cannot parse max bound `{max_str}` as a float"),
        )
    })?;

    // --- parse the function ------------------------------------------------
    let mut func: ItemFn = parse2(item)?;

    let fn_name = func.sig.ident.to_string();
    let original_block = &func.block;

    let nan_msg = format!("{fn_name} returned NaN");
    let range_msg = format!("{fn_name} returned {{}} (expected {range_str})");

    let min_tok = syn::LitFloat::new(&format!("{min_val}_f32"), proc_macro2::Span::call_site());
    let max_tok = syn::LitFloat::new(&format!("{max_val}_f32"), proc_macro2::Span::call_site());

    // Replace the function body.
    let new_body: syn::Block = syn::parse_quote! {
        {
            let __result = (|| #original_block)();
            debug_assert!(
                !__result.is_nan(),
                #nan_msg
            );
            debug_assert!(
                __result >= #min_tok && __result <= #max_tok,
                #range_msg,
                __result
            );
            __result
        }
    };

    func.block = Box::new(new_body);

    Ok(quote! { #func })
}
