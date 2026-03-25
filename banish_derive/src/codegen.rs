use quote::quote;
use crate::parse_ast::{ Block, State, BanishStmt };


/// Returns a stable identifier for the `max_entry` counter of state `index`.
pub fn entry_counter_ident(index: usize) -> syn::Ident {
    syn::Ident::new(
        &format!("__entry_count_{}", index),
        proc_macro2::Span::call_site(),
    )
}

pub fn generate_state(state: &State, input: &Block, index: usize,
    isolated_indices: &[usize]) -> proc_macro2::TokenStream {
    let attrs: &crate::parse_ast::StateAttrs = &state.attrs;

    // Block-level `#![trace]` enables tracing on every state. State-level
    // `#[trace]` enables it for that state only. Either flag activates tracing.
    let trace: bool = attrs.trace || input.attrs.trace;
    
    // Build the trace prefix once: "[banish]" or "[banish:name]".
    let trace_prefix: String = match &input.attrs.id {
        Some(id) => format!("[banish:{}]", id),
        None => "[banish]".to_string(),
    };

    let rules = state.rules.iter().map(|func: &crate::parse_ast::Rule| {
        let rule_name: String = func.name.to_string();
        let body = func.body.iter().map(|stmt: &BanishStmt| generate_stmt(stmt, input));
        let fallback_body = func.fallback_body.as_ref().map(|eb: &Vec<BanishStmt>| {
            eb.iter().map(|stmt: &BanishStmt| generate_stmt(stmt, input))
        });

        let rule_body: proc_macro2::TokenStream = if let Some(condition) = &func.condition {
            if let Some(fallback_body) = fallback_body {
                if trace {
                    quote! {
                        let __cond = #condition;
                        ::banish::log::trace!(
                            "{} rule `{}`: condition = {}",
                            #trace_prefix, #rule_name, __cond
                        );
                        if __cond {
                            __interaction = true;
                            #(#body)*
                        } else { #(#fallback_body)* }
                    }
                } else {
                    quote! {
                        if #condition {
                            __interaction = true;
                            #(#body)*
                        } else { #(#fallback_body)* }
                    }
                }
            } else {
                if trace {
                    quote! {
                        let __cond = #condition;
                        ::banish::log::trace!(
                            "{} rule `{}`: condition = {}",
                            #trace_prefix, #rule_name, __cond
                        );
                        if __cond {
                            __interaction = true;
                            #(#body)*
                        }
                    }
                } else {
                    quote! {
                        if #condition {
                            __interaction = true;
                            #(#body)*
                        }
                    }
                }
            }
        } else {
            // Conditionless rule. Always fires once (guarded by __first_iteration).
            if trace {
                quote! {
                    if __first_iteration {
                        ::banish::log::trace!("{} rule `{}`: unconditional (firing)", #trace_prefix, #rule_name);
                        __interaction = true;
                        #(#body)*
                    }
                }
            } else {
                quote! {
                    if __first_iteration {
                        __interaction = true;
                        #(#body)*
                    }
                }
            }
        };

        rule_body
    });

    // Build the body of the inner loop.
    // After max_iter exhaustion the loop breaks, falling through to the scheduler advance below.
    let loop_body: proc_macro2::TokenStream = {
        let iter_limit: proc_macro2::TokenStream = if let Some((max, redirect)) = &attrs.max_iter {
            let on_exhaust: proc_macro2::TokenStream = match redirect {
                Some(target) => {
                    let target_index: usize = input.states
                        .iter()
                        .position(|s: &State| &s.name == target)
                        .expect("`max_iter` state transition not found. Should have been caught by validate_transition_targets");
                    let target_lit: syn::Index = syn::Index::from(target_index);
                    quote! {
                        __current_state = #target_lit;
                        continue 'banish_main;
                    }
                }
                None => quote! { break; },
            };
            quote! {
                if __first_iteration { __first_iteration = false; }
                __iter_count += 1;
                if !__interaction || __iter_count >= #max { #on_exhaust }
            }
        } else {
            quote! {
                if __first_iteration { __first_iteration = false; }
                if !__interaction { break; }
            }
        };

        let iter_counter_init: proc_macro2::TokenStream = if attrs.max_iter.is_some() {
            quote! { let mut __iter_count: usize = 0; }
        } else { quote! {} };

        let trace_entry: proc_macro2::TokenStream = if trace {
            let state_name: String = state.name.to_string();
            quote! { ::banish::log::trace!("{} entering state `{}`", #trace_prefix, #state_name); }
        } else { quote! {} };

        quote! {
            #trace_entry
            #iter_counter_init
            let mut __first_iteration = true;
            loop {
                __interaction = false;
                #(#rules)*
                #iter_limit
            }
        }
    };

    // Build the max_entry guard (runs before the loop).
    let entry_guard: proc_macro2::TokenStream = if let Some((max, redirect)) = &attrs.max_entry {
        let counter: syn::Ident = entry_counter_ident(index);
        let on_exhaust: proc_macro2::TokenStream = match redirect {
            Some(target) => {
                let target_index: usize = input.states
                    .iter()
                    .position(|s: &State| &s.name == target)
                    .expect("`max_entry` state transition not found. Should have been caught by validate_transition_targets");
                let target_lit: syn::Index = syn::Index::from(target_index);
                quote! {
                    __current_state = #target_lit;
                    continue 'banish_main;
                }
            }
            None => quote! { return; },
        };
        quote! {
            if #counter >= #max { #on_exhaust }
            #counter += 1;
        }
    } else { quote! {} };

    // After leaving this state, advance __current_state past isolated indices.
    // Omitted when max_iter has a redirect. In that case the loop always exits
    // via `continue 'banish_main` and never falls through to here.
    let has_max_iter_redirect = matches!(&attrs.max_iter, Some((_, Some(_))));
    let scheduler_advance: proc_macro2::TokenStream = if has_max_iter_redirect {
        quote! {}
    } else if isolated_indices.is_empty() {
        quote! { __current_state += 1; }
    } else {
        quote! {
            __current_state += 1;
            while [#(#isolated_indices),*].contains(&__current_state) {
                __current_state += 1;
            }
        }
    };

    // The final non-isolated state never needs to advance the scheduler. Any
    // legitimate exit is via a user return or transition, which the validator
    // above enforces. All other states advance normally.
    let state_vars = state.vars.iter().map(|s| quote! { #s });
    let index_lit: syn::Index = syn::Index::from(index);
    quote! {
        #index_lit => {
            #(#state_vars)*
            #entry_guard
            #loop_body
            #scheduler_advance
        }
    }
}

pub fn generate_stmt(stmt: &BanishStmt, input: &Block) -> proc_macro2::TokenStream {
    match stmt {
        BanishStmt::Rust(stmt) => quote! { #stmt },
        BanishStmt::StateTransition(transition) => {
            let target: usize = input.states
                .iter()
                .position(|state: &State| &state.name == transition)
                .expect("Transition target not found. Should have been caught by validate_transition_targets");

            let target: syn::Index = syn::Index::from(target);
            quote! {
                __current_state = #target;
                continue 'banish_main;
            }
        }
        BanishStmt::GuardedStateTransition(transition, guard) => {
            let target: usize = input.states
                .iter()
                .position(|state: &State| &state.name == transition)
                .expect("Transition target not found. Should have been caught by validate_transition_targets");
 
            let target: syn::Index = syn::Index::from(target);
            quote! {
                if #guard {
                    __current_state = #target;
                    continue 'banish_main;
                }
            }
        }
    }
}