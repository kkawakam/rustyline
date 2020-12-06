use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Completer)]
pub fn completer_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = quote! {
        impl #impl_generics ::rustyline::completion::Completer for #name #ty_generics #where_clause {
            type Candidate = ::std::string::String;
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_derive(Helper)]
pub fn helper_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = quote! {
        impl #impl_generics ::rustyline::Helper for #name #ty_generics #where_clause {
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_derive(Highlighter)]
pub fn highlighter_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = quote! {
        impl #impl_generics ::rustyline::highlight::Highlighter for #name #ty_generics #where_clause {
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_derive(Hinter)]
pub fn hinter_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = quote! {
        impl #impl_generics ::rustyline::hint::Hinter for #name #ty_generics #where_clause {
            type Hint = ::std::string::String;
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_derive(Validator)]
pub fn validator_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = quote! {
        impl #impl_generics ::rustyline::validate::Validator for #name #ty_generics #where_clause {
        }
    };
    TokenStream::from(expanded)
}
