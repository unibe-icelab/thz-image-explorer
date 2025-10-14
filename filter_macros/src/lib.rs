extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, ItemStruct};
#[proc_macro_derive(CopyStaticFields, attributes(static_field))]
pub fn derive_copy_static_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Collect fields with #[static_field]
    let fields = match &input.data {
        Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => fields_named
                .named
                .iter()
                .filter(|f| f.attrs.iter().any(|a| a.path().is_ident("static_field")))
                .map(|f| f.ident.as_ref().unwrap())
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };

    let copy_fields = fields.iter().map(|ident| {
        quote! { self.#ident = other.#ident.clone(); }
    });

    let expanded = quote! {
        impl CopyStaticFieldsTrait for #name {
            fn copy_static_fields_from(&mut self, other: &dyn CopyStaticFieldsTrait) {
                if let Some(other) = other.as_any().downcast_ref::<Self>() {
                    #(#copy_fields)*
                }
            }
        }
        impl #name {
            pub fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
    TokenStream::from(expanded)
}

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
