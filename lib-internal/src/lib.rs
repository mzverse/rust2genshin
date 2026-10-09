extern crate alloc;

use alloc::boxed::Box;
use proc_macro::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{parse_macro_input, visit_mut, Block, Expr, ExprAwait, ExprForLoop, ExprLoop, ExprWhile, ExprYield, ForeignItemFn, Item, ItemEnum, ItemFn, ReturnType, Type};
use zyn::zyn;

#[proc_macro_attribute]
pub fn native(_args: TokenStream, input: TokenStream) -> TokenStream {
    if let Ok(ForeignItemFn { attrs, vis, modifiers, sig, .. }) = syn::parse(input.clone()) {
        let block = TokenStream::from(quote! {
            {
                ::core::unreachable!();
            }
        });
        let item = ItemFn {
            attrs,
            vis,
            modifiers,
            sig,
            block: Box::new(parse_macro_input!(block as Block)),
        };
        quote! {
            #[allow(unused_variables)]
            #[inline(never)]
            #[rustc_no_mir_inline]
            #item
        }.into()
    } else {
        tag(input)
    }
}

#[proc_macro_attribute]
pub fn native_calc(args: TokenStream, input: TokenStream) -> TokenStream {
    native(args, input)
}

#[proc_macro_attribute]
pub fn native_exec(args: TokenStream, input: TokenStream) -> TokenStream {
    native(args, input)
}

#[proc_macro_attribute]
pub fn native_enum(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = tag(input);
    let item = parse_macro_input!(input as ItemEnum);
    let ident = item.ident.clone();
    quote! {
        #[repr(i32)]
        #[derive(Clone, Copy, Eq)]
        #item
        impl ::core::cmp::PartialEq for #ident {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                unsafe {
                    crate::native_enum_eq(*self, *other)
                }
            }
        }
    }.into()
}

#[proc_macro_attribute]
pub fn event(_args: TokenStream, input: TokenStream) -> TokenStream {
    tag(input)
}

#[proc_macro_attribute]
pub fn event_listener(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = proc_macro2::TokenStream::from(tag(input));
    quote! {
        #[allow(unused_attributes)]
        #[unsafe(no_mangle)]
        #item
    }.into()
}

fn tag(input: TokenStream) -> TokenStream {
    input.into_iter().map(|mut x| {
        x.set_span(Span::call_site());
        x
    }).collect()
}

#[proc_macro_attribute]
pub fn async_loop(_args: TokenStream, input: TokenStream) -> TokenStream {
    // TODO
    let mut expr = parse_macro_input!(input as Expr);
    let body = match &mut expr {
        Expr::Loop(ExprLoop { body, .. }) => body,
        Expr::ForLoop(ExprForLoop { body, .. }) => body,
        Expr::While(ExprWhile { body, .. }) => body,
        _ => panic!(),
    };
    body.stmts.push(syn::parse(zyn! {
        yield ::rust2genshin_lib::asynchronous::async_jump();
    }.into_token_stream().into()).unwrap());
    expr.into_token_stream().into()
}

#[proc_macro_attribute]
pub fn asynchronous(_args: TokenStream, input: TokenStream) -> TokenStream {
    struct Visitor(Option<syn::Error>);
    impl VisitMut for Visitor {
        fn visit_expr_mut(&mut self, node: &mut Expr) {
            if let Expr::Await(ExprAwait { attrs, base, dot_token, await_token }) = node {
                *node = match syn::parse(zyn! {
                    @for (x in attrs) { {{ x }} }
                    {
                        async fn a() {
                            async {} {{ dot_token }} {{ await_token }}
                        }
                        let mut x = {{ base }};
                        loop {
                            match {
                                use ::rust2genshin_lib::asynchronous::Resume;
                                x.resume()
                            } {
                                ::core::ops::CoroutineState::Yielded(x) => yield x,
                                ::core::ops::CoroutineState::Complete(x) => break x,
                            }
                        }
                    }
                }.into_token_stream().into()) {
                    Ok(x) => x,
                    Err(x) => {
                        self.0 = x.into();
                        return;
                    }
                };
            } else {
                visit_mut::visit_expr_mut(self, node);
            }
        }
        fn visit_expr_yield_mut(&mut self, i: &mut ExprYield) {
            self.0 = syn::Error::new(i.span(), "Do not yield").into()
        }
        fn visit_item_mut(&mut self, _node: &mut Item) {
            // do nothing
        }
    }

    let ItemFn {
        attrs, vis, modifiers, mut sig, mut block
    } = parse_macro_input!(input as ItemFn);
    let mut visitor = Visitor(None);
    visitor.visit_block_mut(&mut block);
    if let Some(err) = visitor.0 {
        return err.into_compile_error().into();
    }
    if sig.asyncness.is_some() {
        return syn::Error::new(sig.span(), "fn must not be async").into_compile_error().into();
    }
    let output = match sig.output {
        ReturnType::Default => {
            let x = quote! { () }.into();
            parse_macro_input!(x as Type)
        },
        ReturnType::Type(_, x) => *x,
    };
    sig.output = ReturnType::Type(Default::default(), syn::parse(zyn! {
        impl ::core::ops::Coroutine<Yield = f32>
    }.to_token_stream().into()).unwrap());
    zyn! {
        @for (x in attrs) { {{x}} }
        {{ vis }}
        {{ modifiers.defaultness }}
        {{ sig }} {
            #[coroutine]
            static move || -> {{ output }}
            {{ block }}
        }
    }.into_token_stream().into()
}
