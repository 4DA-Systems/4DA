use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse2, punctuated::Punctuated, Expr, ExprLit, ItemConst, Lit, LitFloat, Token,
};

/// Expand `#[threshold(min, max)]` on a const declaration.
///
/// Emits the original const unchanged plus a companion const-assertion unit
/// that fails compilation when the value is outside `[min, max]`.
pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    // --- parse arguments ---------------------------------------------------
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args = syn::parse::Parser::parse2(parser, attr)?;

    if args.len() != 2 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "threshold: expected exactly 2 arguments -- #[threshold(min, max)]",
        ));
    }

    let min_lit = extract_float_lit(&args[0], "threshold: min must be a float literal")?;
    let max_lit = extract_float_lit(&args[1], "threshold: max must be a float literal")?;

    // --- parse the const item ----------------------------------------------
    let item_const: ItemConst = parse2(item)?;

    let const_name = &item_const.ident;
    let const_value = &item_const.expr;
    let const_ty = &item_const.ty;

    // Determine the float suffix for the boundary literals so they match the
    // declared type.  We accept `f32` and `f64`.
    let ty_str = quote!(#const_ty).to_string().replace(' ', "");
    let suffix = match ty_str.as_str() {
        "f32" | "f64" => ty_str.clone(),
        _ => {
            return Err(syn::Error::new_spanned(
                const_ty,
                format!("threshold: unsupported type `{ty_str}` — only f32 and f64 are allowed"),
            ));
        }
    };

    // Build suffixed boundary tokens (e.g. `0.20_f32`).
    let min_suffixed: LitFloat =
        LitFloat::new(&format!("{}_{}", min_lit.base10_digits(), suffix), min_lit.span());
    let max_suffixed: LitFloat =
        LitFloat::new(&format!("{}_{}", max_lit.base10_digits(), suffix), max_lit.span());

    // Stringify for error messages.
    let val_str = quote!(#const_value).to_string();
    let name_str = const_name.to_string();
    let min_str = min_lit.base10_digits().to_string();
    let max_str = max_lit.base10_digits().to_string();

    let below_msg = format!("{name_str} value {val_str} is below minimum {min_str}");
    let above_msg = format!("{name_str} value {val_str} is above maximum {max_str}");

    // Generate a check-constant name like `_THRESHOLD_CHECK_CONTEXT_THRESHOLD`.
    let check_ident = syn::Ident::new(
        &format!("_THRESHOLD_CHECK_{}", name_str.to_uppercase()),
        const_name.span(),
    );

    Ok(quote! {
        #item_const

        const #check_ident: () = {
            assert!(#const_value >= #min_suffixed, #below_msg);
            assert!(#const_value <= #max_suffixed, #above_msg);
        };
    })
}

/// Extract a `LitFloat` from an `Expr`, producing `msg` on failure.
fn extract_float_lit(expr: &Expr, msg: &str) -> syn::Result<LitFloat> {
    match expr {
        Expr::Lit(ExprLit { lit: Lit::Float(f), .. }) => Ok(f.clone()),
        // Allow unsuffixed integer literals like `0` or `1` by re-parsing as float.
        Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) => {
            let float_str = format!("{}.0", i.base10_digits());
            Ok(LitFloat::new(&float_str, i.span()))
        }
        _ => Err(syn::Error::new_spanned(expr, msg)),
    }
}
