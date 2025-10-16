use proc_macro::TokenStream;

mod modal;

#[proc_macro]
pub fn define_modal(input: TokenStream) -> TokenStream {
    modal::define_modal(input)
}
