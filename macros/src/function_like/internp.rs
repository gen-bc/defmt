use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, LitStr};

use crate::construct;

pub(crate) fn expand(args: TokenStream) -> TokenStream {
    let literal = parse_macro_input!(args as LitStr);

    let var_addr = if cfg!(feature = "unstable-test") {
        quote!({ defmt::export::fetch_add_string_index() as u16 })
    } else {
        let var_name = quote::format_ident!("S");
        let var_item = construct::static_variable(
            &var_name,
            &literal.value(),
            "prim",
            Some("prim"),
            &parse_quote!(defmt),
        );
        quote!({
            #var_item
            &#var_name as *const u8 as u16
        })
    };

    quote!({
        defmt::export::make_istr(#var_addr)
    })
    .into()
}
