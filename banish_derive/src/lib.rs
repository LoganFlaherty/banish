//! # Banish
//! Banish is a declarative DSL for building rule-based state machines in Rust.
//! States evaluate their rules until reaching a fixed point or triggering a transition,
//! reducing control flow boilerplate.
//! This is the macro implementation for the `banish` crate, which provides the public API
//! and user-facing documentation.

use proc_macro;
use proc_macro2::{ Delimiter, Group, Punct, Spacing, Span, TokenTree };
use quote::quote;
use syn::parse_macro_input;

mod parse_ast;
mod validate;
mod codegen;

use parse_ast::Block;
use validate::{
    validate_state_and_rule_names, validate_transition_targets,
    validate_final_state_has_exit, validate_isolated_states
};
use codegen::{ entry_counter_ident, generate_state };

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
    
    let expanded: proc_macro2::TokenStream;
    if input.attrs.is_async {
        expanded = quote! {
            async move {
                #(#block_vars)*
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
                #(#block_vars)*
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

/// Setup attribute for functions whose body contains a `banish! { }` block.
///
/// `#[banish::machine]` takes no arguments and does two things automatically:
///
/// * Sets `id` in the block attribute to the function name, so trace output is
///   labelled without any extra boilerplate. Can be overridden by writing
///   `#![id = "name"]` inside the `banish!` block explicitly.
///
/// * Injects `async` into the block attribute when applied to an `async fn`,
///   so `#![async]` does not need to be written manually. Writing it explicitly
///   is also fine. The attribute detects it and skips injection.
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
    // This macro takes no arguments
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[banish::machine] takes no arguments. \
            Write block attributes inside the `banish! { }` block with `#![...]`",
        )
        .to_compile_error()
        .into();
    }

    // Get function
    let mut func: syn::ItemFn = match syn::parse(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };
    let fn_name: String = func.sig.ident.to_string();
    let is_async: bool = func.sig.asyncness.is_some();
 
    // Find the banish! invocation in the function body and rewrite its tokens
    let mut found = false;
    for stmt in &mut func.block.stmts {
        let mac: Option<&mut syn::Macro> = match stmt {
            syn::Stmt::Macro(m) => Some(&mut m.mac),
            syn::Stmt::Expr(syn::Expr::Macro(m), _) => Some(&mut m.mac),
            syn::Stmt::Local(local) => {
                if let Some(init) = &mut local.init {
                    find_banish_mac(&mut init.expr)
                } else { None }
            }
            _ => None,
        };
 
        if let Some(mac) = mac {
            if mac.path.is_ident("banish") {
                let body_tts: Vec<proc_macro2::TokenTree> =
                    mac.tokens.clone().into_iter().collect();
                mac.tokens = inject_block_attrs(body_tts, is_async, &fn_name);
                found = true;
                break;
            }
        }
    }
 
    if !found {
        return syn::Error::new(
            func.sig.ident.span(),
            format!(
                "Function `{}` has #[banish::machine] but no `banish! {{}}` invocation in its body",
                fn_name
            ),
        )
        .to_compile_error()
        .into();
    }

    // For async functions, ensure the banish! expression is awaited. This is
    // done as a second pass so the token rewrite above stays focused on attrs
    if is_async {
        wrap_banish_in_await(&mut func.block.stmts);
    }
 
    quote! { #func }.into()
}


//// Helpers

/// Wraps the `banish!` expression in `.await` if it is not already awaited.
/// Handles standalone statements, tail expressions, and let initializers.
fn wrap_banish_in_await(stmts: &mut Vec<syn::Stmt>) {
    for stmt in stmts.iter_mut() {
        match stmt {
            // Standalone: banish! { }; which converts the whole stmt to an awaited expr stmt
            syn::Stmt::Macro(sm) if sm.mac.path.is_ident("banish") => {
                let semi = sm.semi_token;
                let mac_expr = syn::Expr::Macro(syn::ExprMacro {
                    attrs: std::mem::take(&mut sm.attrs),
                    mac: sm.mac.clone(),
                });
                *stmt = syn::Stmt::Expr(
                    syn::Expr::Await(syn::ExprAwait {
                        attrs: vec![],
                        base: Box::new(mac_expr),
                        dot_token: Default::default(),
                        await_token: Default::default(),
                    }),
                    semi,
                );
                return;
            }
            // Expr statement or tail expression: banish! { }
            syn::Stmt::Expr(expr, _) => {
                match expr {
                    // Not yet awaited? Wrap it
                    syn::Expr::Macro(m) if m.mac.path.is_ident("banish") => {
                        let existing = std::mem::replace(
                            expr,
                            syn::Expr::Verbatim(proc_macro2::TokenStream::new()),
                        );
                        *expr = syn::Expr::Await(syn::ExprAwait {
                            attrs: vec![],
                            base: Box::new(existing),
                            dot_token: Default::default(),
                            await_token: Default::default(),
                        });
                        return;
                    }
                    // Already awaited? leave it alone.
                    syn::Expr::Await(_) => return,
                    _ => {}
                }
            }
            // let binding: let x = banish! { }; or let x = banish! { }.await;
            syn::Stmt::Local(local) => {
                if let Some(init) = &mut local.init {
                    match &*init.expr {
                        // Not yet awaited? Wrap it
                        syn::Expr::Macro(m) if m.mac.path.is_ident("banish") => {
                            let existing = std::mem::replace(
                                &mut *init.expr,
                                syn::Expr::Verbatim(proc_macro2::TokenStream::new()),
                            );
                            *init.expr = syn::Expr::Await(syn::ExprAwait {
                                attrs: vec![],
                                base: Box::new(existing),
                                dot_token: Default::default(),
                                await_token: Default::default(),
                            });
                            return;
                        }
                        // Already awaited? Leave it alone.
                        syn::Expr::Await(_) => return,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Drills into an expression to find a `banish!` macro invocation, looking
/// through wrapping expressions that are transparent to the macro's identity
/// such as `.await` and parentheses.
fn find_banish_mac(expr: &mut syn::Expr) -> Option<&mut syn::Macro> {
    match expr {
        syn::Expr::Macro(m) => Some(&mut m.mac),
        syn::Expr::Await(a) => find_banish_mac(&mut a.base),
        syn::Expr::Paren(p) => find_banish_mac(&mut p.expr),
        _ => None,
    }
}

/// Scans the token list from the interior of a `banish! { }` invocation for an
/// existing `#![...]` block attribute and injects missing `async` and `id`
/// entries. If no block attribute is present, prepends a fresh one containing
/// whatever is needed.
///
/// The banish parser requires the block attribute to appear before the first
/// state, so only the head of the token list is inspected.
fn inject_block_attrs(
    mut tts: Vec<proc_macro2::TokenTree>,
    is_async: bool,
    fn_name: &str,
) -> proc_macro2::TokenStream {
    // Look for the `# ! [...]` pattern in the first three tokens
    let has_block_attr = tts.len() >= 3
        && matches!(&tts[0], TokenTree::Punct(p) if p.as_char() == '#')
        && matches!(&tts[1], TokenTree::Punct(p) if p.as_char() == '!')
        && matches!(&tts[2], TokenTree::Group(g) if g.delimiter() == Delimiter::Bracket);
 
    if has_block_attr {
        let inner: proc_macro2::TokenStream = match &tts[2] {
            TokenTree::Group(g) => g.stream(),
            _ => unreachable!(),
        };
 
        let inner_tts: Vec<proc_macro2::TokenTree> = inner.into_iter().collect();
 
        // Only inject what the user has not already written explicitly
        let has_async: bool = inner_tts.iter().any(|tt| {
            matches!(tt, TokenTree::Ident(i) if i.to_string() == "async")
        });
        let has_id: bool = inner_tts.iter().any(|tt| {
            matches!(tt, TokenTree::Ident(i) if i.to_string() == "id")
        });
 
        let mut prefix: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
        if is_async && !has_async { prefix.extend(quote! { async, }); }
        if !has_id { prefix.extend(quote! { id = #fn_name, }); }
 
        let existing: proc_macro2::TokenStream = inner_tts.into_iter().collect();
        let new_inner: proc_macro2::TokenStream = prefix.into_iter().chain(existing).collect();
        tts[2] = TokenTree::Group(Group::new(Delimiter::Bracket, new_inner));
 
        tts.into_iter().collect()
    } else {
        // No block attribute present. Build one containing only what is needed
        let mut attrs: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
        if is_async { attrs.extend(quote! { async, }); }
        attrs.extend(quote! { id = #fn_name });
 
        let hash = TokenTree::Punct(Punct::new('#', Spacing::Alone));
        let bang = TokenTree::Punct(Punct::new('!', Spacing::Alone));
        let group = TokenTree::Group(Group::new(Delimiter::Bracket, attrs));
 
        // Preserve the span of the first existing token so errors point somewhere useful
        let span = tts.first().map(|tt| tt.span()).unwrap_or(Span::call_site());
        let mut prefix = vec![hash, bang, group];
        for tt in &mut prefix { tt.set_span(span); }
 
        prefix.into_iter().chain(tts).collect()
    }
}