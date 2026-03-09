use proc_macro2::TokenTree;
use syn::{
    Expr, Ident, LitInt, Result, Stmt, Token, braced, bracketed,
    parse::{Parse, ParseStream},
};

//// AST

pub struct Context {
    pub states: Vec<State>,
}

pub struct State {
    pub name: Ident,
    pub attrs: StateAttrs,
    pub rules: Vec<Rule>,
}

/// Parsed attributes that can be placed on a state with `#[...]`.
///
/// # Supported attributes
/// 
/// * `isolate` — The state is removed from implicit sequential scheduling.
///   It can only be entered via an explicit `=> @state_name` transition.
///   Isolated states are excluded from the "final state must return" check.
///
/// * `max_iter = N` — Caps the internal fixed-point loop to N iterations.
///   If the loop has not converged by then, the state exits normally (advances
///   to the next non-isolated state). An optional redirect target can be specified
///   with `max_iter = N => @state`, which transitions to `@state` on exhaustion
///   instead of falling through to the scheduler.
///
/// * `max_entry = N` — Limits the number of times this state may be entered
///   across the lifetime of the machine. On the (N+1)th entry, the state
///   immediately executes a `return` without evaluating any rules.
///
/// * `trace` — Emits [`log::trace!`] diagnostics when the state is entered and
///   before each rule is evaluated, showing whether the rule condition fired.
///   Requires a [`log`]-compatible backend; [`env_logger`] is the simplest option:
///   ```rust,ignore
///   // Run with logging enabled:
///   // (bash / zsh) RUST_LOG=trace cargo run -q 2> trace.log
///   // (powershell) $env:RUST_LOG="trace"; cargo run -q 2> trace.log
///   env_logger::init();
///   ```
///
/// Attributes can be combined freely: `#[isolate, max_iter = 5, trace]`
#[derive(Default)]
pub struct StateAttrs {
    pub isolate: bool,
    /// `(iteration_cap, optional_state_transition_on_exhaustion)`
    pub max_iter: Option<(usize, Option<Ident>)>,
    pub max_entry: Option<usize>,
    pub trace: bool,
}

pub struct Rule {
    pub name: Ident,
    pub condition: Option<Expr>,
    pub body: Vec<BanishStmt>,
    pub else_body: Option<Vec<BanishStmt>>,
}

pub enum BanishStmt {
    Rust(Stmt),
    StateTransition(Ident),
}


//// Parsing

impl Parse for Context {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut states: Vec<State> = Vec::with_capacity(2);
        while !input.is_empty() { states.push(input.parse()?); }

        Ok(Context { states })
    }
}

impl Parse for State {
    fn parse(input: ParseStream) -> Result<Self> {
        // Parse optional attribute block: #[attr, key = val, ...]
        let attrs: StateAttrs = if input.peek(Token![#]) {
            input.parse::<Token![#]>()?;
            let content: syn::parse::ParseBuffer<'_>;
            bracketed!(content in input);
            parse_state_attrs(&content)?
        } else { StateAttrs::default() };

        // Reject a second attribute block on the same state.
        if input.peek(Token![#]) {
            return Err(input.error("A state may only have one attribute block `#[...]`"));
        }

        // Parse state name
        input.parse::<Token![@]>()?;
        let name: Ident = input.parse()?;

        // Parse rules until the next state (or its attribute block) or end of input
        let mut rules: Vec<Rule> = Vec::with_capacity(1);
        while !input.is_empty() && !input.peek(Token![@]) && !input.peek(Token![#]) {
            rules.push(input.parse()?);
        }

        Ok(State { name, attrs, rules })
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


//// Helper Functions

/// Parse the comma-separated list of attributes inside `#[...]`.
fn parse_state_attrs(content: &syn::parse::ParseBuffer) -> Result<StateAttrs> {
    let mut attrs: StateAttrs = StateAttrs::default();

    while !content.is_empty() {
        let key: Ident = content.parse()?;
        match key.to_string().as_str() {
            "isolate" => {
                if attrs.isolate {
                    return Err(syn::Error::new(key.span(), "Duplicate attribute `isolate`"));
                }
                attrs.isolate = true;
            }
            "trace" => {
                if attrs.trace {
                    return Err(syn::Error::new(key.span(), "Duplicate attribute `trace`"));
                }
                attrs.trace = true;
            }
            "max_iter" => {
                if attrs.max_iter.is_some() {
                    return Err(syn::Error::new(key.span(), "Duplicate attribute `max_iter`"));
                }
                content.parse::<Token![=]>()?;
                let lit: LitInt = content.parse()?;
                let val: usize = lit.base10_parse::<usize>().map_err(|_| {
                    syn::Error::new(lit.span(), "`max_iter` value must be a positive integer")
                })?;
                if val == 0 {
                    return Err(syn::Error::new(
                        lit.span(),
                        "`max_iter` value must be greater than zero",
                    ));
                }
                // Optional redirect: `max_iter = N => @state`
                let redirect: Option<Ident> = if content.peek(Token![=>]) {
                    content.parse::<Token![=>]>()?;
                    content.parse::<Token![@]>()?;
                    Some(content.parse::<Ident>()?)
                } else { None };
                attrs.max_iter = Some((val, redirect));
            }
            "max_entry" => {
                if attrs.max_entry.is_some() {
                    return Err(syn::Error::new(
                        key.span(),
                        "Duplicate attribute `max_entry`",
                    ));
                }
                content.parse::<Token![=]>()?;
                let lit: LitInt = content.parse()?;
                let val: usize = lit.base10_parse::<usize>().map_err(|_| {
                    syn::Error::new(lit.span(), "`max_entry` value must be a positive integer")
                })?;
                if val == 0 {
                    return Err(syn::Error::new(
                        lit.span(),
                        "`max_entry` value must be greater than zero",
                    ));
                }
                attrs.max_entry = Some(val);
            }
            other => {
                return Err(syn::Error::new(
                    key.span(),
                    format!(
                        "Unknown state attribute `{other}`. \
                         Expected attribute(s): `isolate`, `max_iter`, `max_entry`, `trace`"
                    ),
                ));
            }
        }

        // Consume optional trailing comma between attributes.
        if !content.is_empty() {
            content.parse::<Token![,]>()?;
        }
    }

    Ok(attrs)
}

fn parse_rule_condition(input: &syn::parse::ParseBuffer) -> Result<Option<Expr>> {
    if input.peek(syn::token::Brace) { Ok(None) }
    else {
        let mut cond_tokens: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
        
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
        } else {
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