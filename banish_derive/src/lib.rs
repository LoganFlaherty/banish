//! # Banish
//! Banish is a declarative DSL for building rule-driven state machines in Rust. 
//! It allows you to define states and rules that execute until they reach a stable 
//! fixed point or trigger transitions, making complex control flow easier to express and reason about.
//! This is the macro implementation for the `banish` crate, which provides the public API and user-facing documentation.

use proc_macro;
use proc_macro2::TokenTree;
use quote::quote;
use syn::{
    Expr, Ident, Result, Stmt, Token, braced,
    parse::{Parse, ParseStream}, parse_macro_input,
};
use std::collections::HashSet;


//// AST

struct Context {
    states: Vec<State>,
}

struct State {
    name: Ident,
    rules: Vec<Rule>,
}

struct Rule {
    name: Ident,
    condition: Option<Expr>,
    body: Vec<BanishStmt>,
    else_body: Option<Vec<BanishStmt>>,
}

enum BanishStmt {
    Rust(Stmt),
    StateTransition(Ident),
}


//// Parsing

impl Parse for Context {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut states: Vec<State> = Vec::with_capacity(2);
        while !input.is_empty() {
            states.push(input.parse()?);
        }

        Ok(Context { states })
    }
}

impl Parse for State {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Token![@]>()?;
        let name: Ident = input.parse()?;

        let mut rules: Vec<Rule> = Vec::with_capacity(1);
        while !input.is_empty() && !input.peek(Token![@]) {
            rules.push(input.parse()?);
        }

        Ok(State { name, rules })
    }
}

impl Parse for Rule {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
    
        input.parse::<Token![?]>()?;
        let condition: Option<Expr> = parse_rule_condition(input)?;

        let content: syn::parse::ParseBuffer<'_>;
        braced!(content in input);

        let body: Vec<BanishStmt> = parse_rule_block(&content)?;
        let else_body: Option<Vec<BanishStmt>> = parse_rule_else_block(input)?;

        // If there is an '!?' clause, there must be a condition.
        if condition.is_none() && else_body.is_some() {
            return Err(syn::Error::new(
                name.span(),
                format!(
                    "Rule '{}' cannot have an '!?' clause without a condition.",
                    name
                ),
            ));
        }

        Ok(Rule { name, condition, body, else_body })
    }
}


//// Code Generation

#[proc_macro]
pub fn banish(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: Context = parse_macro_input!(input as Context);

    if let Err(err) = validate_state_and_rule_names(&input) {
        return err.to_compile_error().into();
    }

    let state_blocks = input.states.iter()
        .enumerate().map(|(index, state)| generate_state(state, &input, index));

    let expanded: proc_macro2::TokenStream = quote! {
        (move || {
            let mut __current_state: usize = 0;
            let mut __interaction: bool = false;
            'banish_main: loop {
                match __current_state {
                    #(#state_blocks)*
                    _ => {
                        panic!("Error: No return in final state");
                    },
                }
            }
        })()
    };

    proc_macro::TokenStream::from(expanded)
}


//// Helper Functions

fn parse_rule_condition(input: &syn::parse::ParseBuffer) -> Result<Option<Expr>> {
    if input.peek(syn::token::Brace) {
        Ok(None)
    } else {
        let mut cond_tokens = proc_macro2::TokenStream::new();
        
        // Loop until the start of the body block
        while !input.peek(syn::token::Brace) {
            if input.is_empty() {
                return Err(input.error("Unexpected end of input, expected rule body '{'"));
            }
            // Pull one token at a time
            cond_tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
        }
        
        Ok(Some(syn::parse2(cond_tokens)?))
    }
}

fn parse_rule_block(content: &syn::parse::ParseBuffer) -> Result<Vec<BanishStmt>> {
    let mut body: Vec<BanishStmt> = Vec::new();

    while !content.is_empty() {
        if content.peek(Token![=>]) {
            content.parse::<Token![=>]>()?;
            content.parse::<Token![@]>()?;
            let state: Ident = content.parse()?;
            content.parse::<Token![;]>()?;
            body.push(BanishStmt::StateTransition(state));
        }
        else {
            let stmt: Stmt = content.parse()?;
            body.push(BanishStmt::Rust(stmt));
        }
    }

    Ok(body)
}

fn parse_rule_else_block(input: &syn::parse::ParseBuffer) -> Result<Option<Vec<BanishStmt>>> {
    if input.peek(Token![!]) {
        input.parse::<Token![!]>()?;
        input.parse::<Token![?]>()?;

        let else_content: syn::parse::ParseBuffer<'_>;
        braced!(else_content in input);
        Ok(Some(parse_rule_block(&else_content)?))
    } else { Ok(None) }
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

fn generate_state(state: &State, input: &Context, index: usize) -> proc_macro2::TokenStream {
    let rules = state.rules.iter().map(|func| {
        let body = func.body.iter().map(|stmt| generate_stmt(stmt, &input));
        let else_body = func.else_body.as_ref().map(|else_block| {
            else_block.iter().map(|stmt| generate_stmt(stmt, &input))
        });

        // If a rule has a condition, we want to run it every iteration until the condition is false.
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
        // If a rule is conditionless, we want to run it only once per state.
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