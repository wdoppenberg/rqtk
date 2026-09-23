use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use rqtk_core::verification::{
    build_requirements_doc_from_manifest_dir, build_verification_doc,
    find_activity_from_manifest_dir,
};
use syn::{LitStr, parse_macro_input};

/// Asserts at compile time that a verification activity ID exists in the requirements tree.
///
/// Apply to a `#[test]` function or a `mod` block to link it to a specific
/// `VerificationActivity` in any requirement TOML file under the configured requirements
/// directory from the nearest `.rqtk/config.toml` found by walking up from `CARGO_MANIFEST_DIR`.
///
/// ```rust,ignore
/// #[verifies("VA-SYS-001-01")]
/// #[test]
/// fn lifecycle_round_trip() { /* … */ }
/// ```
///
/// The build will fail with a descriptive error if the activity ID is not found.
/// Hovering over the annotated item shows the requirement overview and activity details.
#[proc_macro_attribute]
pub fn verifies(attr: TokenStream, item: TokenStream) -> TokenStream {
    let activity_id = parse_macro_input!(attr as LitStr);
    let id_value = activity_id.value();

    match find_activity_from_manifest_dir(&id_value) {
        Ok(Some(info)) => {
            let doc = build_verification_doc(&id_value, &info);
            // `include_bytes!` makes Cargo rebuild when the requirement file changes, so the
            // check and the injected docs never go stale.
            let path = info.path.display().to_string();
            let track = quote! { const _: &[u8] = include_bytes!(#path); };
            match syn::parse::<syn::ItemFn>(item.clone()) {
                Ok(mut func) => {
                    let stmt: syn::Stmt = syn::parse_quote! { #track };
                    func.block.stmts.insert(0, stmt);
                    TokenStream::from(quote! {
                        #[doc = #doc]
                        #func
                    })
                }
                Err(_) => {
                    let item_ts: proc_macro2::TokenStream = item.into();
                    TokenStream::from(quote! {
                        #track
                        #[doc = #doc]
                        #item_ts
                    })
                }
            }
        }
        Ok(None) => {
            let msg = format!(
                "verification activity `{}` not found in any requirement file under the configured requirements directory",
                id_value
            );
            TokenStream::from(syn::Error::new(Span::call_site(), msg).to_compile_error())
        }
        Err(e) => {
            let msg = format!(
                "rqtk-macros: could not search requirements for `{}`: {}",
                id_value, e
            );
            TokenStream::from(syn::Error::new(Span::call_site(), msg).to_compile_error())
        }
    }
}

/// Generates a structured rustdoc requirements page from `rqtk` requirements at compile time.
///
/// Apply to a module (or any item that can carry doc attributes):
///
/// ```rust,ignore
/// #[requirements_docs]
/// pub mod requirements {}
/// ```
///
/// The macro loads and validates requirements by walking up from `CARGO_MANIFEST_DIR`
/// until `.rqtk/config.toml` is found. Generation fails with a compile error if requirements
/// are missing or invalid.
#[proc_macro_attribute]
pub fn requirements_docs(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        let msg = "requirements_docs does not accept any attribute arguments";
        return TokenStream::from(syn::Error::new(Span::call_site(), msg).to_compile_error());
    }

    match build_requirements_doc_from_manifest_dir() {
        Ok(doc) => {
            let item_ts: proc_macro2::TokenStream = item.into();
            TokenStream::from(quote! {
                #[doc = #doc]
                #item_ts
            })
        }
        Err(e) => {
            let msg = format!("rqtk-macros: could not generate requirements docs: {}", e);
            TokenStream::from(syn::Error::new(Span::call_site(), msg).to_compile_error())
        }
    }
}
