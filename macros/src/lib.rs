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

    let id_value = if has_id_field {
        quote! { Some(self.id.clone()) }
    } else {
        quote! { None }
    };
    let user_value = if has_user_field {
        quote! { Some(&self.user) }
    } else {
        quote! { None }
    };
    let ts_value = if has_ts_field {
        quote! { Some(self.ts) }
    } else {
        quote! { None }
    };
    let v_value = if has_v_field {
        quote! { Some(self.v.clone()) }
    } else {
        quote! { None }
    };

    let expanded = quote! {
        #[typetag::serde]
        impl Event for #ident {
            fn id(&self) -> Option<String> {
                #id_value
            }

            fn name(&self) -> String {
                self.name.clone()
            }

            fn data(&self) -> &dyn std::any::Any {
                &self.data
            }

            fn user(&self) -> Option<&dyn std::any::Any> {
                #user_value
            }

            fn timestamp(&self) -> Option<i64> {
                #ts_value
            }

            fn version(&self) -> Option<String> {
                #v_value
            }

        }
    };

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
