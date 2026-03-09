use syn::{ Expr, Stmt };
use std::collections::HashSet;
use crate::parse_ast::{ Context, BanishStmt };


/// Validates that state names are unique across the machine, and that rule
/// names are unique within each state.
pub fn validate_state_and_rule_names(input: &Context) -> syn::Result<()> {
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

/// Validates that every transition target. Both `=> @state` in rule bodies and
/// `max_iter = N => @state` in attributes. Refers to a declared state name.
/// Errors point at the ident span of the unknown target.
pub fn validate_transition_targets(input: &Context) -> syn::Result<()> {
    let known: HashSet<String> = input.states.iter()
        .map(|s: &crate::parse_ast::State| s.name.to_string())
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
        // Check max_entry redirect target, if present.
        if let Some((_, Some(redirect))) = &state.attrs.max_entry {
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
pub fn validate_final_state_has_exit(input: &Context) -> syn::Result<()> {
    if let Some(state) = input.states.iter().rev().find(|s: &&crate::parse_ast::State| !s.attrs.isolate) {
        let has_exit = state.rules.iter().any(|rule| {
            let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
                .any(|stmt: &BanishStmt| match stmt {
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
///   `=> @state` in its rules, a `max_iter = N => @state` redirect, or a
///   `max_entry = N => @state` redirect.
///   Without one, the state has no way to terminate.
///
/// * `max_entry` without a redirect is meaningless on an isolated state since
///   the scheduler never re-enters it implicitly — only explicit transitions do,
///   and those are unbounded by design. `max_entry = N => @state` is permitted
///   because it provides a defined exit path.
pub fn validate_isolated_states(input: &Context) -> syn::Result<()> {
    for state in input.states.iter().filter(|s: &&crate::parse_ast::State| s.attrs.isolate) {

        if matches!(&state.attrs.max_entry, Some((_, None))) {
            return Err(syn::Error::new(
                state.name.span(),
                format!(
                    "Isolated state '{}' cannot use `max_entry` without a redirect. Isolated \
                     states are only entered via explicit transitions, not the scheduler. \
                     Use `max_entry = N => @state` to redirect on exhaustion instead",
                    state.name
                ),
            ));
        }

        let has_exit_in_rules: bool = state.rules.iter().any(|rule: &crate::parse_ast::Rule| {
            let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
                .any(|stmt: &BanishStmt| match stmt {
                    BanishStmt::StateTransition(_) => true,
                    BanishStmt::Rust(Stmt::Expr(Expr::Return(_), _)) => true,
                    _ => false,
                });
            check_stmts(&rule.body)
            || rule.else_body.as_ref().map_or(false, check_stmts)
        });

        let has_max_iter_redirect: bool = matches!(&state.attrs.max_iter, Some((_, Some(_))));
        let has_max_entry_redirect: bool = matches!(&state.attrs.max_entry, Some((_, Some(_))));

        if !has_exit_in_rules && !has_max_iter_redirect && !has_max_entry_redirect {
            return Err(syn::Error::new(
                state.name.span(),
                format!(
                    "Isolated state '{}' has no exit. Add a `return`, `=> @state` transition, \
                     `max_iter = N => @state` redirect, or `max_entry = N => @state` redirect",
                    state.name
                ),
            ));
        }
    }

    Ok(())
}