use proc_macro::TokenStream;

#[proc_macro_derive(Event)]
pub fn derive_event_trait(_item: TokenStream) -> TokenStream {
    TokenStream::new()
}
