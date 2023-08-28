use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(InngestEvent)]
pub fn derive_event_trait(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let ident = input.ident.clone();
    // let has_id_field = has_field(&input, "id");
    let has_name_field = has_field(&input, "name");
    let has_data_field = has_field(&input, "data");
    // let has_user_field = has_field(&input, "user");
    // let has_ts_field = has_field(&input, "ts");
    // let has_v_field = has_field(&input, "v");

    if !has_name_field || !has_data_field {
        panic!("name and data fields are required");
    }

    // let id_value: Option<String> = None;
    // let user_value: Option<&dyn std::any::Any> = None;
    // let ts_value: Option<i64> = None;
    // let v_value: Option<String> = None;

    let expanded = quote! {
        #[typetag::serde]
        impl Event for #ident {
            fn id(&self) -> Option<String> {
                // #id_value
                None
            }

            fn name(&self) -> String {
                self.name.clone()
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
