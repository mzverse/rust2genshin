#![no_std]

extern crate alloc;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Item, ItemFn};

#[proc_macro_attribute]
pub fn native(_args: TokenStream, input: TokenStream) -> TokenStream {
    no_inline(tag(input))
}

#[proc_macro_attribute]
pub fn native_calc(_args: TokenStream, input: TokenStream) -> TokenStream {
    no_inline(tag(input))
}

#[proc_macro_attribute]
pub fn native_exec(_args: TokenStream, input: TokenStream) -> TokenStream {
    no_inline(tag(input))
}

#[proc_macro_attribute]
pub fn event_listener(_args: TokenStream, input: TokenStream) -> TokenStream {
    tag(input)
}

fn no_inline(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as Item);
    quote! {
        #[inline(never)]
        #input
    }.into()
}

fn tag(input: TokenStream) -> TokenStream {
    let mut item: ItemFn = syn::parse(input).unwrap();
    item.sig.fn_token = syn::Token![fn](Span::call_site());
    item.sig.ident.set_span(Span::call_site());
    quote::quote!(#item).into()
}
