extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct};

#[proc_macro_attribute]
pub fn register_filter(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;

    // Convert struct name to snake_case
    let fn_name_str = heck::ToSnakeCase::to_snake_case(struct_name.to_string().as_str());
    let fn_name = syn::Ident::new(
        &format!("register_filter_{}", fn_name_str),
        struct_name.span(),
    );

    let expanded = quote! {
        // The original struct definition
        #input

        // Registration function that adds the filter to the global registry
        #[ctor::ctor] // This attribute ensures this function runs when the program starts
        fn #fn_name() {
            crate::filters::filter::FilterRegistry::register_filter::<#struct_name>();  // Register the filter
        }
    };

    TokenStream::from(expanded)
}
