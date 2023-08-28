use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(InngestEvent)]
pub fn derive_event_trait(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let ident = input.ident.clone();
    let has_id_field = has_field(&input, "id");
    let has_name_field = has_field(&input, "name");
    let has_data_field = has_field(&input, "data");
    let has_user_field = has_field(&input, "user");
    let has_ts_field = has_field(&input, "ts");
    let has_v_field = has_field(&input, "v");

    if !has_name_field || !has_data_field {
        panic!("name and data fields are required");
    }

    let expanded = quote! {
        #[typetag::serde]
        impl Event for #ident {
            fn id(&self) -> Option<String> {
                if #has_id_field {
                    Some(self.id.clone())
                } else {
                    None
                }
            }

            fn name(&self) -> String {
                self.name.clone()
            }

            fn data(&self) -> &dyn std::any::Any {
                &self.data
            }

            fn user(&self) -> Option<&dyn std::any::Any> {
                if #has_user_field {
                    Some(&self.user)
                } else {
                    None
                }
            }

            fn timestamp(&self) -> Option<i64> {
                if #has_ts_field {
                    Some(self.ts)
                } else {
                    None
                }
            }

            fn version(&self) -> Option<String> {
                if #has_v_field {
                    Some(self.v.clone())
                } else {
                    None
                }
            }

        }
    };

    // TokenStream::new()
    TokenStream::from(expanded)
}

fn has_field(input: &DeriveInput, field_name: &str) -> bool {
    if let syn::Data::Struct(data) = &input.data {
        data.fields.iter().any(|field| {
            if let Some(ident) = &field.ident {
                ident.to_string() == field_name
            } else {
                false
            }
        })
    } else {
        false
    }
}
