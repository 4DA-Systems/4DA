use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, Data, DeriveInput, Fields, Type};

/// Expand `#[derive(ScoringBuilder)]` on a struct.
///
/// Generates:
/// - `StructName::builder() -> StructNameBuilder`
/// - `StructNameBuilder` with a fluent setter for each field
/// - `StructNameBuilder::build(self) -> StructName`
///
/// All fields receive sensible defaults (0 for numbers, empty for
/// collections, `None` for options, `false` for bools).
pub(crate) fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = parse2(item)?;
    let struct_ident = &input.ident;
    let builder_ident = format_ident!("{}Builder", struct_ident);
    let vis = &input.vis;

    let fields = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    struct_ident,
                    "ScoringBuilder: only named-field structs are supported",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                struct_ident,
                "ScoringBuilder: can only be derived on structs",
            ));
        }
    };

    // Collect per-field data.
    let mut builder_fields = Vec::new();
    let mut setter_fns = Vec::new();
    let mut build_assignments = Vec::new();

    for field in fields.iter() {
        let field_ident = field
            .ident
            .as_ref()
            .expect("ScoringBuilder: expected named field");
        let field_ty = &field.ty;
        let default_expr = default_for_type(field_ty);

        builder_fields.push(quote! {
            #field_ident: #field_ty
        });

        setter_fns.push(quote! {
            pub fn #field_ident(mut self, v: #field_ty) -> Self {
                self.#field_ident = v;
                self
            }
        });

        build_assignments.push(quote! {
            #field_ident: self.#field_ident
        });

        // Use default_expr for the builder's Default impl.
        // We'll collect defaults separately below.
        let _ = default_expr;
    }

    // Defaults for the builder constructor.
    let default_values: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let ident = f.ident.as_ref().unwrap();
            let default = default_for_type(&f.ty);
            quote! { #ident: #default }
        })
        .collect();

    Ok(quote! {
        impl #struct_ident {
            /// Create a builder with default values for all fields.
            pub fn builder() -> #builder_ident {
                #builder_ident {
                    #(#default_values),*
                }
            }
        }

        #vis struct #builder_ident {
            #(#builder_fields),*
        }

        impl #builder_ident {
            #(#setter_fns)*

            /// Consume the builder and produce the final struct.
            pub fn build(self) -> #struct_ident {
                #struct_ident {
                    #(#build_assignments),*
                }
            }
        }
    })
}

/// Produce a default-value token stream for common Rust types.
///
/// Recognises:
/// - `bool` -> `false`
/// - `i8..i128`, `u8..u128`, `isize`, `usize` -> `0`
/// - `f32`, `f64` -> `0.0`
/// - `String` -> `String::new()`
/// - `Vec<_>` -> `Vec::new()`
/// - `HashMap<_, _>` / `BTreeMap<_, _>` -> `<Type>::new()`
/// - `Option<_>` -> `None`
/// - Anything else -> `Default::default()` (fallback)
fn default_for_type(ty: &Type) -> TokenStream {
    let ty_str = quote!(#ty).to_string().replace(' ', "");

    // Bool
    if ty_str == "bool" {
        return quote! { false };
    }

    // Integer primitives
    let int_types = [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
    ];
    if int_types.contains(&ty_str.as_str()) {
        return quote! { 0 };
    }

    // Float primitives
    if ty_str == "f32" || ty_str == "f64" {
        return quote! { 0.0 };
    }

    // String
    if ty_str == "String" {
        return quote! { String::new() };
    }

    // Vec<...>
    if ty_str.starts_with("Vec<") {
        return quote! { Vec::new() };
    }

    // HashMap<...>
    if ty_str.starts_with("HashMap<") || ty_str.starts_with("std::collections::HashMap<") {
        return quote! { std::collections::HashMap::new() };
    }

    // BTreeMap<...>
    if ty_str.starts_with("BTreeMap<") || ty_str.starts_with("std::collections::BTreeMap<") {
        return quote! { std::collections::BTreeMap::new() };
    }

    // HashSet<...>
    if ty_str.starts_with("HashSet<") || ty_str.starts_with("std::collections::HashSet<") {
        return quote! { std::collections::HashSet::new() };
    }

    // Option<...>
    if ty_str.starts_with("Option<") {
        return quote! { None };
    }

    // Fallback — rely on the type implementing Default.
    quote! { Default::default() }
}
