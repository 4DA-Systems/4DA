use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, Data, DeriveInput, Expr, ExprArray, ExprLit, Fields, Lit, MetaNameValue};

/// Expand `#[confirmation_gate(axes = ["context", "interest", ...])]` on a struct.
///
/// Validates that each axis has a corresponding `{axis}_confirmed: bool` field
/// and generates `AXIS_COUNT` and `AXIS_NAMES` associated constants.
pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    // --- parse attribute ---------------------------------------------------
    let meta: MetaNameValue = parse2(attr)?;

    let path_str = meta
        .path
        .get_ident()
        .map(|id| id.to_string())
        .unwrap_or_default();
    if path_str != "axes" {
        return Err(syn::Error::new_spanned(
            &meta.path,
            "confirmation_gate: expected `axes = [\"...\", ...]`",
        ));
    }

    let axes = extract_string_array(&meta.value)?;
    if axes.is_empty() {
        return Err(syn::Error::new_spanned(
            &meta.value,
            "confirmation_gate: axes array must not be empty",
        ));
    }

    // --- parse the struct --------------------------------------------------
    let input: DeriveInput = parse2(item.clone())?;
    let struct_ident = &input.ident;

    let fields = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    struct_ident,
                    "confirmation_gate: only named-field structs are supported",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                struct_ident,
                "confirmation_gate: can only be applied to structs",
            ));
        }
    };

    // Collect field names for validation.
    let field_names: Vec<String> = fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|id| id.to_string()))
        .collect();

    // Validate that each axis has a matching `{axis}_confirmed` bool field.
    for axis in &axes {
        let expected_field = format!("{axis}_confirmed");
        if !field_names.contains(&expected_field) {
            return Err(syn::Error::new_spanned(
                struct_ident,
                format!(
                    "confirmation_gate: axis \"{axis}\" requires a field `{expected_field}: bool`, \
                     but it was not found"
                ),
            ));
        }

        // Check that the field type is bool.
        let field = fields
            .iter()
            .find(|f| f.ident.as_ref().map(|id| id.to_string()) == Some(expected_field.clone()))
            .unwrap();

        let field_ty = &field.ty;
        let field_ty_str = quote!(#field_ty).to_string().replace(' ', "");
        if field_ty_str != "bool" {
            return Err(syn::Error::new_spanned(
                &field.ty,
                format!(
                    "confirmation_gate: field `{expected_field}` must be `bool`, found `{}`",
                    field_ty_str
                ),
            ));
        }
    }

    let axis_count = axes.len();
    let axis_strs: Vec<&str> = axes.iter().map(|s| s.as_str()).collect();

    // Re-parse the struct item to preserve its original tokens (attributes, vis, etc.).
    let struct_item: proc_macro2::TokenStream = item;

    Ok(quote! {
        #struct_item

        impl #struct_ident {
            pub const AXIS_COUNT: usize = #axis_count;
            pub const AXIS_NAMES: &'static [&'static str] = &[#(#axis_strs),*];
        }
    })
}

/// Extract a `Vec<String>` from an array expression like `["a", "b", "c"]`.
fn extract_string_array(expr: &Expr) -> syn::Result<Vec<String>> {
    match expr {
        Expr::Array(ExprArray { elems, .. }) => {
            let mut result = Vec::with_capacity(elems.len());
            for elem in elems {
                match elem {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) => {
                        result.push(s.value());
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            elem,
                            "confirmation_gate: each axis must be a string literal",
                        ));
                    }
                }
            }
            Ok(result)
        }
        _ => Err(syn::Error::new_spanned(
            expr,
            "confirmation_gate: axes value must be an array literal like [\"a\", \"b\"]",
        )),
    }
}
