extern crate alloc;

use alloc::boxed::Box;
use proc_macro::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{Block, ForeignItemFn, ItemEnum, ItemFn, ReturnType, Type, parse_macro_input};
use zyn::zyn;

#[proc_macro_attribute]
pub fn native(_args: TokenStream, input: TokenStream) -> TokenStream {
    if let Ok(ForeignItemFn { attrs, vis, sig, .. }) = syn::parse(input.clone()) {
        let block = TokenStream::from(quote! {
            {
                ::core::unreachable!();
            }
        });
        let item = ItemFn {
            attrs,
            vis,
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
pub fn asynchronous(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ItemFn {
        attrs, vis, mut sig, block
    } = parse_macro_input!(input as ItemFn);
    assert!(sig.asyncness.is_none());
    let output = match sig.output {
        ReturnType::Default => {
            let x = quote! { () }.into();
            parse_macro_input!(x as Type)
        },
        ReturnType::Type(_, x) => *x,
    };
    let output = zyn! {
        impl ::core::ops::Coroutine<Yield = f32, Return = {{ output }}>
    }.to_token_stream().into();
    sig.output = ReturnType::Type(Default::default(), parse_macro_input!(output as Type).into());
    zyn! {
        @for (x in attrs) { {{x}} }
        {{ vis }}
        {{ sig }} {
            #[coroutine]
            static move ||
            {{ block }}
        }
    }.into_token_stream().into()
}
