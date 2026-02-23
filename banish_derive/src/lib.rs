//! # Banish
//! Banish is a declarative DSL for building rule-based state machines in Rust. 
//! States evaluate their rules until reaching a fixed point or triggering a transition, reducing control flow boilerplate. 
//! This is the macro implementation for the `banish` crate, which provides the public API and user-facing documentation.

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
    if let Err(err) = validate_final_state_has_return_or_transition(&input) {
        return err.to_compile_error().into();
    }

    // Generate code for each state and its rules
    let state_blocks = input.states.iter()
        .enumerate().map(|(index, state)| generate_state(state, &input, index));

    let expanded: proc_macro2::TokenStream = quote! {
        (move || {
            let mut __current_state: usize = 0;
            let mut __interaction: bool = false;
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

fn validate_final_state_has_return_or_transition(input: &Context) -> syn::Result<()> {
    if let Some(state) = input.states.last() {
        let has_return = state.rules.iter().any(|rule| {
            let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
            .any(|stmt| match stmt {
                BanishStmt::StateTransition(_) => true,
                BanishStmt::Rust(Stmt::Expr(Expr::Return(_), _)) => true,
                _ => false,
            });

            check_stmts(&rule.body)
            || rule.else_body.as_ref().map_or(false, check_stmts)
        });

        if !has_return {
            return Err(syn::Error::new(
                state.name.span(),
                format!("Final state '{}' must have a return or state transition statement", state.name),
            ));
        }
    }
    Ok(())
}

fn generate_state(state: &State, input: &Context, index: usize) -> proc_macro2::TokenStream {
    let rules = state.rules.iter().map(|func| {
        let body = func.body.iter().map(|stmt| generate_stmt(stmt, &input));
        let else_body = func.else_body.as_ref().map(|else_block| {
            else_block.iter().map(|stmt| generate_stmt(stmt, &input))
        });

        // If a rule has a condition, we want to run it every iteration until the condition is false
        if let Some(condition) = &func.condition {
            if let Some(else_body) = else_body {
                quote! {
                    if #condition {
                        __interaction = true;
                        #(#body)*
                    } else {
                        #(#else_body)*
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
        // If a rule is conditionless, we want to run it only once per state
        else {
            quote! {
                if __first_iteration {
                    __interaction = true;
                    #(#body)*
                }
            }
        }
    });

    // State loop
    // If no interactions occur in a full pass, exit state
    let index: syn::Index = syn::Index::from(index);
    quote! {
        #index => {
            let mut __first_iteration = true;
            loop {
                __interaction = false;
                #(#rules)*
                if __first_iteration { __first_iteration = false; }
                if !__interaction {
                    break;
                }
            }

            __current_state += 1;
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
                .unwrap_or_else(|| { panic!("Error: Invalid state transition target {}", transition); });
            
            let target: syn::Index = syn::Index::from(target);
            quote! {
                __current_state = #target;
                continue 'banish_main;
            }
        }
    }
}