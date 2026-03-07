//! # Banish
//! Banish is a declarative DSL for building rule-based state machines in Rust.
//! States evaluate their rules until reaching a fixed point or triggering a transition,
//! reducing control flow boilerplate.
//! This is the macro implementation for the `banish` crate, which provides the public API
//! and user-facing documentation.

use proc_macro;
use quote::quote;
use syn::{ Expr, Stmt, parse_macro_input };
use std::collections::HashSet;
mod parse_ast;
use parse_ast::{Context, State, BanishStmt};


//// Code Generation

#[proc_macro]
pub fn banish(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Context = parse_macro_input!(input as Context);

    // Error handling
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
            state.attrs.max_entry.map(|_| {
                let ident = entry_counter_ident(i);
                quote! { let mut #ident: usize = 0; }
            })
        })
        .collect();

    // Generate code for each state and its rules
    let state_blocks = input.states.iter()
        .enumerate()
        .map(|(index, state)| generate_state(state, &input, index, &isolated_indices));

    let expanded: proc_macro2::TokenStream = quote! {
        (move || {
            let mut __current_state: usize = 0;
            let mut __interaction: bool = false;
            #(#entry_counters)*
            'banish_main: loop {
                match __current_state {
                    #(#state_blocks)*
                    _ => { unreachable!("banish: invalid state index"); },
                }
            }
        })()
    };

    proc_macro::TokenStream::from(expanded)
}


//// Helper Functions

/// Returns a stable identifier for the `max_entry` counter of state `index`.
fn entry_counter_ident(index: usize) -> syn::Ident {
    syn::Ident::new(
        &format!("__entry_count_{}", index),
        proc_macro2::Span::call_site(),
    )
}

fn validate_state_and_rule_names(input: &Context) -> syn::Result<()> {
    let mut state_names: HashSet<String> = HashSet::new();
    for state in &input.states {
        let name: String = state.name.to_string();
        if !state_names.insert(name.clone()) {
            return Err(syn::Error::new(
                state.name.span(),
                format!("Duplicate state name '{}'", name),
            ));
        }

        let mut rule_names: HashSet<String> = HashSet::new();
        for rule in &state.rules {
            let name: String = rule.name.to_string();
            if !rule_names.insert(name.clone()) {
                return Err(syn::Error::new(
                    rule.name.span(),
                    format!(
                        "Duplicate rule '{}' in state '{}'",
                        name, state.name
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// Validates that every transition target — both `=> @state` in rule bodies and
/// `max_iter = N => @state` in attributes — refers to a declared state name.
/// Errors point at the ident span of the unknown target.
fn validate_transition_targets(input: &Context) -> syn::Result<()> {
    let known: HashSet<String> = input.states.iter()
        .map(|s| s.name.to_string())
        .collect();

    let check = |ident: &syn::Ident| -> syn::Result<()> {
        if !known.contains(&ident.to_string()) {
            Err(syn::Error::new(
                ident.span(),
                format!("Unknown state `{}`", ident),
            ))
        } else { Ok(()) }
    };

    for state in &input.states {
        // Check max_iter redirect target, if present.
        if let Some((_, Some(redirect))) = &state.attrs.max_iter {
            check(redirect)?;
        }

        // Check all => @state transitions in rule bodies and else-bodies.
        for rule in &state.rules {
            for stmt in &rule.body {
                if let BanishStmt::StateTransition(target) = stmt {
                    check(target)?;
                }
            }
            if let Some(else_body) = &rule.else_body {
                for stmt in else_body {
                    if let BanishStmt::StateTransition(target) = stmt {
                        check(target)?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// The final non-isolated state must contain a return or explicit transition so
/// the machine has a defined exit path. Isolated states are excluded because they
/// are never reached by the sequential scheduler and therefore are never "last".
fn validate_final_state_has_exit(input: &Context) -> syn::Result<()> {
    if let Some(state) = input.states.iter().rev().find(|s| !s.attrs.isolate) {
        let has_exit = state.rules.iter().any(|rule| {
            let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
                .any(|stmt| match stmt {
                    BanishStmt::StateTransition(_) => true,
                    BanishStmt::Rust(Stmt::Expr(Expr::Return(_), _)) => true,
                    _ => false,
                });
            check_stmts(&rule.body)
                || rule.else_body.as_ref().map_or(false, check_stmts)
        });

        if !has_exit {
            return Err(syn::Error::new(
                state.name.span(),
                format!(
                    "Final state '{}' must have a return or state transition statement",
                    state.name
                ),
            ));
        }
    }
    Ok(())
}

/// Validates isolated state constraints:
///
/// * Every isolated state must have a defined exit — either a `return` or
///    `=> @state` in its rules, or a `max_iter = N => @state` redirect.
///    Without one, the state has no way to terminate.
///
/// * `max_entry` is meaningless on an isolated state since the scheduler never
///    re-enters it implicitly — only explicit transitions do, and those are
///    unbounded by design.
fn validate_isolated_states(input: &Context) -> syn::Result<()> {
    for state in input.states.iter().filter(|s| s.attrs.isolate) {

        if state.attrs.max_entry.is_some() {
            return Err(syn::Error::new(
                state.name.span(),
                format!(
                    "Isolated state '{}' cannot use `max_entry`. Isolated states are only \
                     entered via explicit transitions, not the scheduler",
                    state.name
                ),
            ));
        }

        let has_exit_in_rules = state.rules.iter().any(|rule| {
            let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
                .any(|stmt| match stmt {
                    BanishStmt::StateTransition(_) => true,
                    BanishStmt::Rust(Stmt::Expr(Expr::Return(_), _)) => true,
                    _ => false,
                });
            check_stmts(&rule.body)
            || rule.else_body.as_ref().map_or(false, check_stmts)
        });

        let has_max_iter_redirect = matches!(&state.attrs.max_iter, Some((_, Some(_))));

        if !has_exit_in_rules && !has_max_iter_redirect {
            return Err(syn::Error::new(
                state.name.span(),
                format!(
                    "Isolated state '{}' has no exit. Add a `return`, `=> @state` transition, \
                     or `max_iter = N => @state` redirect",
                    state.name
                ),
            ));
        }
    }

    Ok(())
}

fn generate_state(
    state: &State,
    input: &Context,
    index: usize,
    isolated_indices: &[usize],
) -> proc_macro2::TokenStream {
    let attrs = &state.attrs;

    let rules = state.rules.iter().map(|func| {
        let rule_name = func.name.to_string();
        let body  = func.body.iter().map(|stmt| generate_stmt(stmt, input));
        let else_body = func.else_body.as_ref().map(|eb| {
            eb.iter().map(|stmt| generate_stmt(stmt, input))
        });

        let rule_body = if let Some(condition) = &func.condition {
            if let Some(else_body) = else_body {
                if attrs.trace {
                    quote! {
                        let __cond = #condition;
                        eprintln!(
                            "[banish: trace] rule `{}`: condition = {}",
                            #rule_name, __cond
                        );
                        if __cond {
                            __interaction = true;
                            #(#body)*
                        } else { #(#else_body)* }
                    }
                } else {
                    quote! {
                        if #condition {
                            __interaction = true;
                            #(#body)*
                        } else { #(#else_body)* }
                    }
                }
            } else {
                if attrs.trace {
                    quote! {
                        let __cond = #condition;
                        eprintln!(
                            "[banish: trace] rule `{}`: condition = {}",
                            #rule_name, __cond
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
            // Conditionless rule — always fires once (guarded by __first_iteration).
            if attrs.trace {
                quote! {
                    if __first_iteration {
                        eprintln!("[banish: trace] rule `{}`: unconditional (firing)", #rule_name);
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
        let iter_limit = if let Some((max, redirect)) = &attrs.max_iter {
            let on_exhaust = match redirect {
                Some(target) => {
                    let target_index: usize = input.states
                        .iter()
                        .position(|s| &s.name == target)
                        .expect("banish: max_iter state transition not found. Should have been caught by validate_transition_targets");
                    let target_lit = syn::Index::from(target_index);
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

        let iter_counter_init = if attrs.max_iter.is_some() {
            quote! { let mut __iter_count: usize = 0; }
        } else { quote! {} };

        let trace_entry = if attrs.trace {
            let state_name = state.name.to_string();
            quote! { eprintln!("[banish: trace] entering state `{}`", #state_name); }
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
    let entry_guard: proc_macro2::TokenStream = if let Some(max) = attrs.max_entry {
        let counter = entry_counter_ident(index);
        quote! {
            if #counter >= #max { return; }
            #counter += 1;
        }
    } else { quote! {} };

    // After leaving this state, advance __current_state past isolated indices.
    // Omitted when max_iter has a redirect — in that case the loop always exits
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

    // The final non-isolated state never needs to advance the scheduler — any
    // legitimate exit is via a user return or transition, which the validator
    // above enforces. All other states advance normally.
    let index_lit: syn::Index = syn::Index::from(index);
    quote! {
        #index_lit => {
            #entry_guard
            #loop_body
            #scheduler_advance
        }
    }
}

fn generate_stmt(stmt: &BanishStmt, input: &Context) -> proc_macro2::TokenStream {
    match stmt {
        BanishStmt::Rust(stmt) => quote! { #stmt },
        BanishStmt::StateTransition(transition) => {
            let target: usize = input.states
                .iter()
                .position(|state| &state.name == transition)
                .expect("Transition target not found. Should have been caught by validate_transition_targets");

            let target: syn::Index = syn::Index::from(target);
            quote! {
                __current_state = #target;
                continue 'banish_main;
            }
        }
    }
}