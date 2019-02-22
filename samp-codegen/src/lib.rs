#![recursion_limit = "128"]

extern crate proc_macro;
use proc_macro::TokenStream;

mod plugin;
mod native;

pub(crate) const NATIVE_PREFIX: &'static str = "__samp_native_";
pub(crate) const REG_PREFIX: &'static str = "__samp_reg_";

#[proc_macro_attribute]
pub fn native(args: TokenStream, input: TokenStream) -> TokenStream {
    native::create_native(args, input)
}

#[proc_macro]
pub fn initialize_plugin(input: TokenStream) -> TokenStream {
    plugin::create_plugin(input)
}