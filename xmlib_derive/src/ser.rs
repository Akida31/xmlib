use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::Ident;

use crate::parse::{Enum, Field, Input, InputData, NamedStruct, UnnamedStruct};

pub(crate) fn expand(Input { data, ident }: Input) -> TokenStream {
    let (pre, inner) = match data {
        InputData::Enum(v) => (Default::default(), expand_enum(v)),
        InputData::NamedStruct(v) => expand_named_struct(v, &ident),
        InputData::UnnamedStruct(v) => (Default::default(), expand_unnamed_struct(v)),
    };

    quote! {
        #pre

        #[automatically_derived]
        impl<W: ::std::io::Write> ::xmlib::ser::Serialize<W> for #ident {
            #[inline]
            fn ser(&self, writer__: &mut ::xmlib::ser::XmlWriter<W>) -> ::std::io::Result<()> {
                #inner
            }
        }
    }
    .into()
}

fn expand_enum(Enum { has_data, variants }: Enum) -> TokenStream2 {
    if has_data {
        let variants = variants.into_iter().map(|(ident, _name, _ty)| {
            quote! {
                Self::#ident(v) => ::xmlib::ser::Serialize::ser(v, writer__),
            }
        });
        quote! {
            match self {
                #(#variants)*
            }
        }
    } else {
        let variants = variants
            .into_iter()
            .map(|(ident, name, _ty)| quote! { Self::#ident => #name, });
        quote! {
            writer__.write_all(match self {
                #(#variants)*
            })
        }
    }
}

// Unnamed structs are just new-types and serialized as them
fn expand_unnamed_struct(_s: UnnamedStruct) -> TokenStream2 {
    quote! { self.0.ser(writer__) }
}

fn expand_named_struct(s: NamedStruct, ident: &Ident) -> (TokenStream2, TokenStream2) {
    let NamedStruct {
        no_constructor,
        raw_ser_name,
        ty_attribute,
        ty_value,
        ty_value_buf,
        ty_collect_namespaces,
    } = s;
    let mut default_params = Vec::new();
    let mut default_inits = Vec::new();
    let mut required_params_doc = String::new();

    let namespace_ser_code = if let Some(ident) = ty_collect_namespaces {
        default_inits.push(quote! {#ident: ::std::default::Default::default()});
        quote! {
            for (name, value) in &self.#ident {
                writer__.write_all(b" ")?;
                writer__.write_all(&name)?;
                writer__.write_all(b"=\"")?;
                writer__.write_all(&value)?;
                writer__.write_all(b"\"")?;
            }
        }
    } else {
        Default::default()
    };

    let mut process_field = |field: &Field| -> (_, _) {
        let ty = &field.ty;

        let default = field.default.as_ref().map(|default| {
            if let syn::Lit::Str(lit) = default {
                match ty {
                    syn::Type::Path(syn::TypePath { qself: None, path })
                        if path.is_ident("String") =>
                    {
                        quote! {::std::string::String::from(#lit)}
                    }
                    _ => syn::parse2(syn::parse_str(&lit.value()).unwrap()).unwrap(),
                }
            } else {
                quote! {#default}
            }
        });

        let ident = &field.ident;

        default_inits.push(
            default
                .as_ref()
                .map(|default| quote! {#ident: #default})
                .unwrap_or_else(|| {
                    required_params_doc.push_str(&format!(
                        "\n{}: [`{}`]",
                        ident,
                        ty.to_token_stream()
                    ));
                    default_params.push(quote! {#ident: #ty});
                    quote! {#ident}
                }),
        );
        let code = quote! {::xmlib::ser::Serialize::ser(&self.#ident, writer__)?;};
        (default, code)
    };

    let attr_ser_code: Vec<_> = ty_attribute
        .into_iter()
        .map(|field| {
            let (default, code) = process_field(&field);
            let start = proc_macro2::Literal::byte_string(format!(" {}=\"", field.name).as_bytes());
            let ident = &field.ident;

            let inner = quote! {
                writer__.write_all(#start)?;
                #code
                writer__.write_all(b"\"")?;
            };

            if let Some(default) = default {
                quote! {
                    if self.#ident != #default {
                        #inner
                    }
                }
            } else {
                inner
            }
        })
        .collect();

    let inner_ser_code = if ty_value.is_empty() && ty_value_buf.is_none() {
        quote! {
            writer__.write_all(b"/>")?;
        }
    } else {
        let values: Vec<_> = ty_value
            .into_iter()
            .chain(ty_value_buf)
            .map(|field| {
                let (default, code) = process_field(&field);
                let ident = field.ident;
                if let Some(default) = default {
                    quote! {
                        if self.#ident != #default {
                            #code
                        }
                    }
                } else {
                    code
                }
            })
            .collect();

        let end = proc_macro2::Literal::byte_string(format!("</{}>", &raw_ser_name).as_bytes());

        quote! {
            writer__.write_all(b">")?;
            #(#values)*
            writer__.write_all(#end)?;
        }
    };

    let tag_start = proc_macro2::Literal::byte_string(format!("<{}", &raw_ser_name).as_bytes());

    let literal_name = ident.to_string();

    let constructor = if no_constructor {
        quote! {}
    } else {
        quote! {
            impl #ident {
                #[doc=concat!(" Create a [`", #literal_name, "`] from required values.\n\n## Required values:", #required_params_doc)]
                #[allow(clippy::too_many_arguments)]
                pub fn with_default(#(#default_params,)*) -> Self {
                    Self {
                       #(#default_inits,)*
                    }
                }
            }
        }
    };

    let inner = quote! {
        writer__.write_all(#tag_start)?;

        #(#attr_ser_code)*
        #namespace_ser_code
        #inner_ser_code

        Ok(())
    };

    (constructor, inner)
}
