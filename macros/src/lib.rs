use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(InngestEvent)]
pub fn derive_event_trait(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let ident = input.ident;
    let mut event = EventFields::default();

    println!("Ident: {:?}", ident);

    // TODO:
    // - [x] retrieve fields from struct
    // - [x] check that `name`, `data` field exists
    // - [x] emit compile error if `name` or `data` is not a field
    // - [ ] check if any of these fields exists (id, user, ts, v)
    match input.data {
        syn::Data::Struct(data) => {
            // println!("Fields: {:#?}", data.fields);
            match data.fields {
                syn::Fields::Named(nf) => {
                    for n in nf.named.iter() {
                        if let Some(ident) = &n.ident {
                            match ident.to_string().as_str() {
                                "name" => {
                                    event.name = true;
                                }
                                "data" => {
                                    event.data = true;
                                }
                                field => {
                                    println!("Unsupported field: {:?}", field);
                                }
                            }
                        }
                    }
                }
                _ => (),
            };
        }
        _ => panic!("Invalid types"),
    }

    if !event.is_valid() {
        println!("Event: {:#?}", event);
        panic!("The event need to have a `name` and `data` field");
    }

    // - [ ] rotate through the list of fields and implement the trait function
    // for each of them appropriately
    let expanded = quote! {
        #[typetag::serde]
        impl Event for #ident {
            fn id(&self) -> Option<String> {
                None
            }

            fn name(&self) -> String {
                "test/event".to_string()
            }

            fn data(&self) -> &dyn std::any::Any {
                &self.data
            }

            fn user(&self) -> Option<&dyn std::any::Any> {
                None
            }

            fn timestamp(&self) -> Option<i64> {
                None
            }

            fn version(&self) -> Option<String> {
                None
            }

        }
    };

    // TokenStream::new()
    TokenStream::from(expanded)
}

#[derive(Debug, Default)]
struct EventFields {
    id: bool,
    name: bool,
    data: bool,
    user: bool,
    ts: bool,
    timestamp: bool,
}

impl EventFields {
    fn is_valid(&self) -> bool {
        self.name && self.data
    }
}
