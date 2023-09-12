use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(InngestEvent, attributes(event_name))]
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

    let none = quote! { None };
    let id_value = if has_id_field {
        quote! { Some(self.id.clone()) }
    } else {
        none.clone()
    };
    let user_value = if has_user_field {
        quote! { Some(&self.user) }
    } else {
        none.clone()
    };
    let ts_value = if has_ts_field {
        quote! { Some(self.ts) }
    } else {
        none.clone()
    };
    let v_value = if has_v_field {
        quote! { Some(self.v.clone()) }
    } else {
        none.clone()
    };

    let evt_name = event_name(&input, "event_name").expect("event_name attribute is required");
    let evt_type = ident.to_string();

    let expanded = quote! {
        #[typetag::serde]
        impl Event for #ident {
            fn id(&self) -> Option<String> {
                #id_value
            }

            fn name(&self) -> String {
                #evt_name.to_string()
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

        inngest::__private::inventory::submit! {
            #ident::inngest_register()
        }

        impl #ident {
            pub const fn inngest_register() -> inngest::__private::EventMeta {
                inngest::__private::EventMeta {
                    ename: #evt_name,
                    etype: #evt_type
                }
            }
        }
    };

    expanded.into()
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

fn event_name(input: &DeriveInput, attr_name: &str) -> Option<String> {
    if let syn::Data::Struct(data) = &input.data {
        for field in data.fields.iter() {
            for attr in field.attrs.iter() {
                if let Some(ident) = attr.meta.path().get_ident() {
                    if ident.to_string() == attr_name {
                        let val: Option<String> = match &attr.meta {
                            syn::Meta::NameValue(meta) => match &meta.value {
                                syn::Expr::Lit(expr) => match &expr.lit {
                                    syn::Lit::Str(s) => Some(s.value()),
                                    _ => None,
                                },
                                _ => None,
                            },
                            _ => None,
                        };

                        return val;
                    }
                }
            }
        }
    }

    None
}
