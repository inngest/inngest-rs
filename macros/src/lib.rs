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

    // TODO:
    // - [ ] retrieve fields from struct
    // - [ ] check that `name`, `data` field exists
    // - [ ] emit compile error if `name` or `data` is not a field
    // - [ ] check if any of these fields exists (id, user, ts, v)
    // - [ ] rotate through the list of fields and implement the trait function
    // for each of them appropriately
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
