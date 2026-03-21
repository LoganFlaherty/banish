use proc_macro2::{ Delimiter, Group, Punct, Spacing, Span, TokenTree };
use quote::quote;

/// Handler fn for `machine` in `lib.rs`
pub fn machine_handler(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
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