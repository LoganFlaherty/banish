//! # Banish
//! Banish is a declarative DSL for building rule-based state machines in Rust.
//! States evaluate their rules until reaching a fixed point or triggering a transition,
//! reducing control flow boilerplate.
//! This is the macro implementation for the `banish` crate, which provides the public API
//! and user-facing documentation.

use proc_macro;
use quote::quote;
use syn::parse_macro_input;

mod parse_ast;
mod validate;
mod codegen;
mod machine;

use parse_ast::Block;
use validate::{
    validate_state_and_rule_names, validate_transition_targets,
    validate_final_state_has_exit, validate_isolated_states,
    validate_no_break_in_final_state
};
use codegen::{ entry_counter_ident, generate_state };
use machine::machine_handler;

/// Expands a banish block into a labeled `match` loop at compile time.
///
/// Parses the input into states and rules, validates names, transition targets,
/// and exit paths, then generates either an immediately invoked closure or an
/// `async move` expression depending on the `#![async]` block attribute.
#[proc_macro]
pub fn banish(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Block = parse_macro_input!(input as Block);

    if let Err(err) = validate_state_and_rule_names(&input) {
        return err.to_compile_error().into();
    }
    if let Err(err) = validate_transition_targets(&input) {
        return err.to_compile_error().into();
    }
    if let Err(err) = validate_final_state_has_exit(&input) {
        return err.to_compile_error().into();
    }
    if let Err(err) = validate_isolated_states(&input) {
        return err.to_compile_error().into();
    }
    if let Err(err) = validate_no_break_in_final_state(&input) {
        return err.to_compile_error().into();
    }

    // Collect the indices of all isolated states so every non-isolated state's
    // fall-through can skip over them
    let isolated_indices: Vec<usize> = input.states.iter()
        .enumerate()
        .filter(|(_, s)| s.attrs.isolate)
        .map(|(i, _)| i)
        .collect();

    // Per-state persistent counters for `max_entry` (declared outside the main loop)
    let entry_counters: Vec<proc_macro2::TokenStream> = input.states.iter()
        .enumerate()
        .filter_map(|(i, state)| {
            state.attrs.max_entry.as_ref().map(|_| {
                let ident: syn::Ident = entry_counter_ident(i);
                quote! { let mut #ident: usize = 0; }
            })
        })
        .collect();

    // Generate code for each state and its rules
    let state_blocks = input.states.iter()
        .enumerate()
        .map(|(index, state)| generate_state(state, &input, index, &isolated_indices));

    // Set entry state. Ignoring isolated states
    let entry_state: usize = input.states.iter()
        .position(|s| !s.attrs.isolate)
        .unwrap_or(0);

    // Block-level variable declarations, emitted before internal state variables
    let block_vars = input.vars.iter().map(|s| quote! { #s });
    
    // When `#![dispatch(expr)]` is present, the initial state is determined at
    // runtime by calling `.variant_name()` on the expression, which returns a
    // `&'static str` with no allocation. The match arms are the state name
    // strings produced at compile time.
    let current_state_init: proc_macro2::TokenStream =
        if let Some(dispatch_expr) = &input.attrs.dispatch {
            let arms: Vec<proc_macro2::TokenStream> = input.states.iter()
                .enumerate()
                .map(|(i, state)| {
                    let name: String = pascal_to_snake(&state.name.to_string());
                    let idx: syn::Index = syn::Index::from(i);
                    quote! { #name => #idx }
                })
                .collect();
            quote! {
                let mut __current_state: usize = match ::banish::BanishDispatch::variant_name(&#dispatch_expr) {
                    #(#arms,)*
                    other => panic!("[banish] dispatch: no state matching variant `{}`", other),
                };
            }
        } else {
            quote! { let mut __current_state: usize = #entry_state; }
        };

    let expanded: proc_macro2::TokenStream;
    if input.attrs.is_async {
        expanded = quote! {
            async move {
                #(#block_vars)*
                #current_state_init
                let mut __interaction: bool = false;
                #(#entry_counters)*
                'banish_main: loop {
                    match __current_state {
                        #(#state_blocks)*
                        _ => { unreachable!("Invalid state index"); },
                    }
                }
            }
        };
    } else {
        expanded = quote! {
            (move || {
                #(#block_vars)*
                #current_state_init
                let mut __interaction: bool = false;
                #(#entry_counters)*
                'banish_main: loop {
                    match __current_state {
                        #(#state_blocks)*
                        _ => { unreachable!("Invalid state index"); },
                    }
                }
            })()
        };
    }

    proc_macro::TokenStream::from(expanded)
}

/// Derive macro for the `BanishDispatch` trait.
///
/// Generates a `variant_name` implementation that returns the snake_case name
/// of the current variant as a `&'static str`. Works on all enum variants
/// regardless of whether they carry data. The data is ignored, only the
/// variant name is used for dispatch.
///
/// # Example
///
/// ```rust
/// use banish::BanishDispatch;
///
/// #[derive(BanishDispatch)]
/// enum PipelineState {
///     Normalize,
///     Finalize,
///     Done,
/// }
///
/// let state = PipelineState::Normalize;
/// assert_eq!(state.variant_name(), "normalize");
/// ```
#[proc_macro_derive(BanishDispatch)]
pub fn derive_banish_dispatch(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: syn::DeriveInput = match syn::parse(input) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error().into(),
    };
 
    let name: &syn::Ident = &input.ident;
 
    let variants: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma> = match &input.data {
        syn::Data::Enum(e) => &e.variants,
        _ => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                "BanishDispatch can only be derived on enums",
            )
            .to_compile_error()
            .into();
        }
    };
 
    let arms: Vec<proc_macro2::TokenStream> = variants.iter().map(|v| {
        let variant_ident: &syn::Ident = &v.ident;
        let snake: String = pascal_to_snake(&variant_ident.to_string());
 
        let pattern = match &v.fields {
            syn::Fields::Unit => quote! { #name::#variant_ident },
            syn::Fields::Unnamed(_) => quote! { #name::#variant_ident(..) },
            syn::Fields::Named(_) => quote! { #name::#variant_ident { .. } },
        };
 
        quote! { #pattern => #snake }
    }).collect();
 
    quote! {
        impl ::banish::BanishDispatch for #name {
            fn variant_name(&self) -> &'static str {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
    .into()
}

/// Setup attribute for functions whose body contains a `banish! { }` block.
///
/// `#[banish::machine]` takes no arguments and does three things automatically:
///
/// * Sets `id` in the block attribute to the function name, so trace output is
///   labelled without any extra boilerplate. Can be overridden by writing
///   `#![id = "name"]` inside the `banish!` block explicitly.
///
/// * Injects `async` into the block attribute when applied to an `async fn`,
///   so `#![async]` does not need to be written manually. Writing it explicitly
///   is also fine. The attribute detects it and skips injection.
/// 
/// * Injects `.await` on the `banish!` expression when the function is async,
///   so the future produced by `#![async]` is driven to completion automatically.
///   If `.await` is already present it is left alone.
///
/// # Example
///
/// ```rust
/// // `async` and id = "fetch" are both injected automatically.
/// #[banish::machine]
/// async fn fetch() -> &'static str {
///     banish! {
///         @check
///             ping? {
///                 let ok = reqwest::get("https://example.com").await.is_ok();
///                 if ok { return "Up"; } else { return "Down"; }
///             }
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn machine(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    machine_handler(attr, item)
}


//// Helpers

/// Converts a PascalCase identifier string to snake_case at compile time,
/// used to produce the match arm strings for `#![dispatch(...)]`.
///
/// `Normalize` -> `normalize`
fn pascal_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i != 0 {
            out.push('_');
        }
        out.extend(ch.to_lowercase());
    }
    out
}