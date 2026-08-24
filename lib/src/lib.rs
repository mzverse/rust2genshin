#![no_std]

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn entry(_args: TokenStream, input: TokenStream) -> TokenStream {
    todo!();
    input
}
