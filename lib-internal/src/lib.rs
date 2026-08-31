extern crate alloc;

use alloc::boxed::Box;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{Block, ForeignItemFn, ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn native(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ForeignItemFn { attrs, vis, sig, .. } = parse_macro_input!(input as ForeignItemFn);
    let block = TokenStream::from(quote! {
        {
            ::core::unreachable!();
        }
    });
    let item = ItemFn {
        attrs, vis, sig,
        block: Box::new(parse_macro_input!(block as Block)),
    };
    quote! {
        #[allow(unused_variables)]
        #[inline(never)]
        #item
    }.into()
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
pub fn event_listener(_args: TokenStream, input: TokenStream) -> TokenStream {
    tag(input)
}

fn tag(input: TokenStream) -> TokenStream {
    let mut item: ItemFn = syn::parse(input).unwrap();
    item.sig.fn_token = syn::Token![fn](Span::call_site());
    item.sig.ident.set_span(Span::call_site());
    item.into_token_stream().into()
}
