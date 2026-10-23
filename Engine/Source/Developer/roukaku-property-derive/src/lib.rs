use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Fields};

#[proc_macro_derive(RoukakuProperty)]
pub fn roukaku_property(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let fields = match input.data {
        syn::Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields
                .named
                .into_iter()
                .map(|field| {
                    let ident = field.ident.expect("named field");
                    let field_name = ident.to_string();
                    let ty = field.ty;
                    quote! {
                        ::roukaku_core::PropertyField::new(#field_name, stringify!(#ty))
                    }
                })
                .collect::<Vec<_>>(),
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "RoukakuProperty supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "RoukakuProperty supports structs only")
                .to_compile_error()
                .into();
        }
    };
    quote! {
        impl #impl_generics ::roukaku_core::PropertyDescribe for #name #ty_generics #where_clause {
            fn property_metadata() -> ::roukaku_core::TypeMetadata {
                ::roukaku_core::TypeMetadata {
                    type_name: stringify!(#name),
                    fields: vec![#(#fields),*],
                }
            }
        }
    }
    .into()
}
