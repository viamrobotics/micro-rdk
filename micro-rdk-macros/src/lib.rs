use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(DoCommand)]
pub fn impl_do_command_default(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let gen = quote! {
        impl #impl_generics DoCommand for #name #ty_generics #where_clause {}
    };
    gen.into()
}
