use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(InngestEvent)]
pub fn derive_event_trait(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let ident = input.ident;
    println!("Ident: {:?}", ident);
    match input.data {
        syn::Data::Struct(data) => {
            println!("Fields: {:#?}", data.fields);
        }
        _ => panic!("Invalid types"),
    }

    let expanded = quote! {
        #[typetag::serde]
        impl Event for #ident {
            fn name(&self) -> String {
                "test/event".to_string()
            }

            fn data(&self) -> &dyn std::any::Any {
                &self.data
            }
        }
    };

    // TokenStream::new()
    TokenStream::from(expanded)
}
