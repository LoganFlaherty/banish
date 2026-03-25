use syn::{ Expr, Stmt };
use std::collections::HashSet;
use crate::parse_ast::{ Block, BanishStmt };


/// Validates that state names are unique across the machine, and that rule
/// names are unique within each state.
pub fn validate_state_and_rule_names(input: &Block) -> syn::Result<()> {
    let mut state_names: HashSet<String> = HashSet::new();
    for state in &input.states {
        let name: String = state.name.to_string();
        if !state_names.insert(name.clone()) {
            return Err(syn::Error::new(
                state.name.span(),
                format!("Duplicate state name `{}`. Rename one of the states", name),
            ));
        }

        let mut rule_names: HashSet<String> = HashSet::new();
        for rule in &state.rules {
            let name: String = rule.name.to_string();
            if !rule_names.insert(name.clone()) {
                return Err(syn::Error::new(
                    rule.name.span(),
                    format!(
                        "Duplicate rule name `{}` in state `{}`. Rename one of the rules",
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
pub fn validate_transition_targets(input: &Block) -> syn::Result<()> {
    let known: HashSet<String> = input.states.iter()
        .map(|s: &crate::parse_ast::State| s.name.to_string())
        .collect();

    let check = |ident: &syn::Ident| -> syn::Result<()> {
        if !known.contains(&ident.to_string()) {
            Err(syn::Error::new(
                ident.span(),
                format!("Unknown state transition `{}`. Declare the state or fix the name", ident),
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
                match stmt {
                    BanishStmt::StateTransition(target) => check(target)?,
                    BanishStmt::GuardedStateTransition(target, _) => check(target)?,
                    _ => {}
                }
            }
            if let Some(fallback_body) = &rule.fallback_body {
                for stmt in fallback_body {
                    match stmt {
                        BanishStmt::StateTransition(target) => check(target)?,
                        BanishStmt::GuardedStateTransition(target, _) => check(target)?,
                        _ => {}
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
pub fn validate_final_state_has_exit(input: &Block) -> syn::Result<()> {
    if let Some(state) = input.states.iter().rev().find(|s: &&crate::parse_ast::State| !s.attrs.isolate) {
        let has_exit = state.rules.iter().any(|rule| {
            let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
                .any(|stmt: &BanishStmt| match stmt {
                    BanishStmt::StateTransition(_) => true,
                    BanishStmt::Rust(Stmt::Expr(expr, _)) => expr_has_return(expr),
                    _ => false,
                });
            check_stmts(&rule.body)
                || rule.fallback_body.as_ref().map_or(false, check_stmts)
        });

        if !has_exit {
            return Err(syn::Error::new(
                state.name.span(),
                format!(
                    "Final state `{}` must have a return or state transition statement. \
                    Add `return` or `=> @state` to at least one rule",
                    state.name
                ),
            ));
        }
    }
    Ok(())
}

/// Validates isolated state constraint:
///
/// Every isolated state must have a defined exit — either a `return` or
/// => @state` in its rules, or a `max_iter = N => @state` redirect.
/// Without one, the state has no way to return control after its fixed point
/// is reached. `max_entry = N => @state` does not count — it only fires on
/// the (N+1)th entry and does nothing to exit entries 1 through N.
pub fn validate_isolated_states(input: &Block) -> syn::Result<()> {
    for state in input.states.iter().filter(|s: &&crate::parse_ast::State| s.attrs.isolate) {

        let has_exit_in_rules: bool = state.rules.iter().any(|rule: &crate::parse_ast::Rule| {
            let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
                .any(|stmt: &BanishStmt| match stmt {
                    BanishStmt::StateTransition(_) => true,
                    BanishStmt::Rust(Stmt::Expr(expr, _)) => expr_has_return(expr),
                    _ => false,
                });
            check_stmts(&rule.body)
            || rule.fallback_body.as_ref().map_or(false, check_stmts)
        });

        let has_max_iter_redirect: bool = matches!(&state.attrs.max_iter, Some((_, Some(_))));

        if !has_exit_in_rules && !has_max_iter_redirect {
            return Err(syn::Error::new(
                state.name.span(),
                format!(
                    "Isolated state `{}` must have a return or state transition statement. \
                    Add a `return`, `=> @state` transition, or `max_iter = N => @state` redirect",
                    state.name
                ),
            ));
        }
    }

    Ok(())
}

/// Recursively checks whether a Rust expression contains a `return` anywhere
/// reachable without crossing a loop or closure boundary, since those create
/// their own control flow scopes.
fn expr_has_return(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) => true,
        Expr::If(e) => {
            block_has_return(&e.then_branch)
                || e.else_branch.as_ref().map_or(false, |(_, eb)| expr_has_return(eb))
        }
        Expr::Block(e) => block_has_return(&e.block),
        Expr::Match(e) => e.arms.iter().any(|arm| expr_has_return(&arm.body)),
        _ => false,
    }
}

fn block_has_return(block: &syn::Block) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        Stmt::Expr(expr, _) => expr_has_return(expr),
        _ => false,
    })
}

/// Recursively checks whether a Rust expression contains a `break` anywhere
/// reachable without crossing a loop or closure boundary. A `break` inside a
/// user-written `for` or `loop` block breaks that inner loop, not the state
/// loop, so those boundaries are not descended into.
fn expr_has_break(expr: &Expr) -> bool {
    match expr {
        Expr::Break(_) => true,
        Expr::If(e) => {
            block_has_break(&e.then_branch)
                || e.else_branch.as_ref().map_or(false, |(_, eb)| expr_has_break(eb))
        }
        Expr::Block(e) => block_has_break(&e.block),
        Expr::Match(e) => e.arms.iter().any(|arm| expr_has_break(&arm.body)),
        _ => false,
    }
}

fn block_has_break(block: &syn::Block) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        Stmt::Expr(expr, _) => expr_has_break(expr),
        _ => false,
    })
}

/// Validates that no rule in the final non-isolated state uses `break`.
///
/// `break` exits the fixed-point loop and falls through to the scheduler
/// advance, which increments `__current_state` past the last valid index
/// and hits `unreachable!()` at runtime. In the final state the only valid
/// exits are `return` and `=> @state`.
pub fn validate_no_break_in_final_state(input: &Block) -> syn::Result<()> {
    let Some(state) = input.states.iter().rev().find(|s: &&crate::parse_ast::State| !s.attrs.isolate)
        else { return Ok(()); };

    for rule in &state.rules {
        let check_stmts = |stmts: &Vec<BanishStmt>| stmts.iter()
            .any(|stmt: &BanishStmt| match stmt {
                BanishStmt::Rust(Stmt::Expr(expr, _)) => expr_has_break(expr),
                _ => false,
            });

        if check_stmts(&rule.body) || rule.fallback_body.as_ref().map_or(false, check_stmts) {
            return Err(syn::Error::new(
                rule.name.span(),
                format!(
                    "Rule `{}` in final state `{}` uses `break`, which exits the \
                    fixed-point loop and advances past the last valid state index, \
                    causing a runtime panic. Use `return` or `=> @state` instead",
                    rule.name, state.name
                ),
            ));
        }
    }

    Ok(())
}