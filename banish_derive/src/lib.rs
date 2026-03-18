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

use parse_ast::Context;
use validate::{ validate_state_and_rule_names, validate_transition_targets,
                validate_final_state_has_exit, validate_isolated_states };
use codegen::{ entry_counter_ident, generate_state };


#[proc_macro]
pub fn banish(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Context = parse_macro_input!(input as Context);

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

    // Collect the indices of all isolated states so every non-isolated state's
    // fall-through can skip over them.
    let isolated_indices: Vec<usize> = input.states.iter()
        .enumerate()
        .filter(|(_, s)| s.attrs.isolate)
        .map(|(i, _)| i)
        .collect();

    // Per-state persistent counters for `max_entry` (declared outside the main loop).
    let entry_counters: Vec<proc_macro2::TokenStream> = input.states.iter()
        .enumerate()
        .filter_map(|(i, state)| {
            state.attrs.max_entry.as_ref().map(|_| {
                let ident: syn::Ident = entry_counter_ident(i);
                quote! { let mut #ident: usize = 0; }
            })
        })
        .collect();

    // Generate code for each state and its rules.
    let state_blocks = input.states.iter()
        .enumerate()
        .map(|(index, state)| generate_state(state, &input, index, &isolated_indices));

    // Set entry state. Ignoring isolated states.
    let entry_state: usize = input.states.iter()
        .position(|s| !s.attrs.isolate)
        .unwrap_or(0);
    
    let expanded: proc_macro2::TokenStream;
    if input.attrs.is_async {
        expanded = quote! {
            async move {
                let mut __current_state: usize = #entry_state;
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
                let mut __current_state: usize = #entry_state;
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