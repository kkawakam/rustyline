use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Index, Path};

fn get_field_by_attr<'a>(data: &'a Data, ident: &str) -> Option<(usize, &'a Field)> {
    if let Data::Struct(struct_data) = &data {
        let mut fields = struct_data.fields.iter().enumerate().filter(|(_, field)| {
            field.attrs.iter().any(|attr| {
                attr.path().is_ident("rustyline")
                    && attr
                        .parse_args::<Path>()
                        .map_or(false, |arg| arg.is_ident(ident))
            })
        });

        let field = fields.next();

        if fields.next().is_some() {
            panic!("Only one {:} field is allowed.", ident);
        }

        field
    } else {
        None
    }
}

fn field_name_or_index_token(index: usize, field: &Field) -> TokenStream2 {
    if let Some(ident) = field.ident.as_ref() {
        quote!(#ident)
    } else {
        let index = Index::from(index);
        quote!(#index)
    }
}

#[proc_macro_derive(Completer, attributes(rustyline))]
pub fn completer_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = if let Some((index, field)) = get_field_by_attr(&input.data, "Completer") {
        let field_name_or_index = field_name_or_index_token(index, field);
        let field_type = &field.ty;

        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::completion::Completer for #name #ty_generics #where_clause {
                type Candidate = <#field_type as ::rustyline::completion::Completer>::Candidate;

                fn complete(
                    &self,
                    line: &str,
                    pos: usize,
                    ctx: &::rustyline::Context<'_>,
                ) -> ::rustyline::Result<(usize, ::std::vec::Vec<Self::Candidate>)> {
                    ::rustyline::completion::Completer::complete(&self.#field_name_or_index, line, pos, ctx)
                }

                fn update(&self, line: &mut ::rustyline::line_buffer::LineBuffer, start: usize, elected: &str, cl: &mut ::rustyline::Changeset) {
                    ::rustyline::completion::Completer::update(&self.#field_name_or_index, line, start, elected, cl)
                }
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::completion::Completer for #name #ty_generics #where_clause {
                type Candidate = ::std::string::String;
            }
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
        #[automatically_derived]
        impl #impl_generics ::rustyline::Helper for #name #ty_generics #where_clause {
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_derive(Highlighter, attributes(rustyline))]
pub fn highlighter_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = if let Some((index, field)) = get_field_by_attr(&input.data, "Highlighter") {
        let field_name_or_index = field_name_or_index_token(index, field);

        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::highlight::Highlighter for #name #ty_generics #where_clause {
                fn highlight<'l>(&self, line: &'l str, pos: usize) -> ::std::borrow::Cow<'l, str> {
                    ::rustyline::highlight::Highlighter::highlight(&self.#field_name_or_index, line, pos)
                }

                fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
                    &'s self,
                    prompt: &'p str,
                    default: bool,
                ) -> ::std::borrow::Cow<'b, str> {
                    ::rustyline::highlight::Highlighter::highlight_prompt(&self.#field_name_or_index, prompt, default)
                }

                fn highlight_hint<'h>(&self, hint: &'h str) -> ::std::borrow::Cow<'h, str> {
                    ::rustyline::highlight::Highlighter::highlight_hint(&self.#field_name_or_index, hint)
                }

                fn highlight_candidate<'c>(
                    &self,
                    candidate: &'c str,
                    completion: ::rustyline::config::CompletionType,
                ) -> ::std::borrow::Cow<'c, str> {
                    ::rustyline::highlight::Highlighter::highlight_candidate(&self.#field_name_or_index, candidate, completion)
                }

                fn highlight_char(&self, line: &str, pos: usize) -> bool {
                    ::rustyline::highlight::Highlighter::highlight_char(&self.#field_name_or_index, line, pos)
                }
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::highlight::Highlighter for #name #ty_generics #where_clause {
            }
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_derive(Hinter, attributes(rustyline))]
pub fn hinter_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = if let Some((index, field)) = get_field_by_attr(&input.data, "Hinter") {
        let field_name_or_index = field_name_or_index_token(index, field);
        let field_type = &field.ty;

        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::hint::Hinter for #name #ty_generics #where_clause {
                type Hint = <#field_type as ::rustyline::hint::Hinter>::Hint;

                fn hint(&self, line: &str, pos: usize, ctx: &::rustyline::Context<'_>) -> ::std::option::Option<Self::Hint> {
                    ::rustyline::hint::Hinter::hint(&self.#field_name_or_index, line, pos, ctx)
                }
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::hint::Hinter for #name #ty_generics #where_clause {
                type Hint = ::std::string::String;
            }
        }
    };
    TokenStream::from(expanded)
}

#[proc_macro_derive(Validator, attributes(rustyline))]
pub fn validator_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let expanded = if let Some((index, field)) = get_field_by_attr(&input.data, "Validator") {
        let field_name_or_index = field_name_or_index_token(index, field);

        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::validate::Validator for #name #ty_generics #where_clause {
                fn validate(
                    &self,
                    ctx: &mut ::rustyline::validate::ValidationContext,
                ) -> ::rustyline::Result<::rustyline::validate::ValidationResult> {
                    ::rustyline::validate::Validator::validate(&self.#field_name_or_index, ctx)
                }

                fn validate_while_typing(&self) -> bool {
                    ::rustyline::validate::Validator::validate_while_typing(&self.#field_name_or_index)
                }
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            impl #impl_generics ::rustyline::validate::Validator for #name #ty_generics #where_clause {
            }
        }
    };
    TokenStream::from(expanded)
}
