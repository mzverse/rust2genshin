extern crate alloc;

use alloc::boxed::Box;
use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::{Block, ForeignItemFn, ItemFn, parse_macro_input};

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
