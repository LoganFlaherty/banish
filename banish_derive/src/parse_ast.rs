use proc_macro2::TokenTree;
use syn::{
    Expr, Ident, Result, Stmt, Token, braced,
    parse::{Parse, ParseStream},
};

//// AST

pub struct Context {
    pub states: Vec<State>,
}

pub struct State {
    pub name: Ident,
    pub rules: Vec<Rule>,
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