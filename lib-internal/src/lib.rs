#![no_std]

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn event_listener(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}
