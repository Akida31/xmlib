use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::Ident;

use crate::parse::{Enum, Field, Input, InputData, NamedStruct, UnnamedStruct};

pub(crate) fn expand(Input { data, ident }: Input) -> TokenStream {
    match data {
        InputData::Enum(v) => expand_enum(v, ident),
        InputData::NamedStruct(v) => expand_named_struct(v, ident),
        InputData::UnnamedStruct(v) => expand_unnamed_struct(v, ident),
    }
}

fn expand_enum(Enum { has_data, variants }: Enum, enum_ident: Ident) -> TokenStream {
    let ident_str = enum_ident.to_string();
    let inner_code = if has_data {
        let mut variants: Vec<_> = variants.into_iter().map(|(ident, _name, ty)| {
            quote! {<#ty as ::xmlib::de::DeserializeBuf>::de_buf(buf).map(|res| Self::#ident(res))}
        }).collect();
        // last variant is different
        let last_variant = variants.pop().unwrap();
        let variants = variants.into_iter().map(|variant| {
            quote! {
                if let ::std::result::Result::Ok(res) = #variant {
                    return ::std::result::Result::Ok(res)
                }
            }
        });

        quote! {
            #(#variants)*
            #last_variant
        }
    } else {
        let variants = variants.into_iter().map(|(ident, name, _ty)| {
            quote! { #name => ::std::result::Result::Ok(Self::#ident), }
        });

        quote! {
            match buf {
                #(#variants)*
                v => ::std::result::Result::Err(::xmlib::de::Error {
                    ty_name: ::std::string::String::from(#ident_str),
                    kind: ::xmlib::de::ErrorKind::InvalidType(
                        format!("invalid type {}",
                            ::std::string::String::from_utf8_lossy(v))
                    )
                }),
            }
        }
    };

    quote! {
        #[automatically_derived]
        impl ::xmlib::de::DeserializeBuf for #enum_ident {
            #[inline]
            fn de_buf(
                buf: &[u8],
            ) -> ::std::result::Result<Self, ::xmlib::de::Error> {
                #inner_code
            }
        }
    }
    .into()
}

// Unnamed structs are just new-types and deserialized as them
fn expand_unnamed_struct(
    UnnamedStruct { validation, ty }: UnnamedStruct,
    struct_ident: Ident,
) -> TokenStream {
    let ident_str = struct_ident.to_string();
    let inner_ident = Ident::new("inner", proc_macro2::Span::call_site());
    let validation =
        validation.map(|validation| create_validation(&validation, &inner_ident, &ident_str));

    quote! {
        #[automatically_derived]
        impl ::xmlib::de::DeserializeBuf for #struct_ident {
            #[inline]
            fn de_buf(
                buf: &[u8],
            ) -> ::std::result::Result<Self, ::xmlib::de::Error> {
                let inner = match #ty::de_buf(buf) {
                    ::std::result::Result::Ok(inner) => inner,
                    ::std::result::Result::Err(e) => return ::std::result::Result::Err(e),
                };

                #validation

                ::std::result::Result::Ok(Self(inner))
            }
        }

    }
    .into()
}

fn expand_named_struct(s: NamedStruct, struct_ident: Ident) -> TokenStream {
    let NamedStruct {
        no_constructor: _,
        raw_ser_name,
        ty_attribute,
        ty_value,
        ty_value_buf,
        ty_collect_namespaces,
    } = s;

    let mut init_code = Vec::new();
    let mut attr_ser_code = Vec::new();
    let mut value_ser_code = Vec::new();
    let mut pre_finish_code = Vec::new();
    let mut validation_code = Vec::new();
    let mut finish_code = Vec::new();

    let mut process_field = |field: &Field| {
        let ident = &field.ident;
        let default = field.default.as_ref().map(|default| {
            if let syn::Lit::Str(lit) = default {
                match &field.ty {
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

        let init_val = if let Some(default) = default.as_ref() {
            default.clone()
        } else {
            if !field.has_multiple {
                let name = &field.name;
                pre_finish_code.push(quote! {
                    let #ident = match #ident {
                        ::std::option::Option::Some(val) => val,
                        ::std::option::Option::None => return ::std::result::Result::Err(
                            ::xmlib::de::Error {
                                ty_name: ::std::string::String::from(#raw_ser_name),
                                kind: ::xmlib::de::ErrorKind::MissingAttr(::std::string::String::from(#name)),
                            }
                        )
                    };
                });
            }
            quote! {::std::default::Default::default()}
        };

        finish_code.push(quote! {#ident, });

        if let Some(validation) = &field.validation {
            validation_code.push(create_validation(validation, ident, &field.name));
        }

        let ty = &field.ty;
        init_code.push(if default.is_some() || field.has_multiple {
            quote! {let mut #ident: #ty = #init_val;}
        } else {
            quote! {let mut #ident: ::std::option::Option<#ty> = #init_val;}
        });

        default
    };

    for field in ty_attribute {
        let default = process_field(&field);
        let name_str = proc_macro2::Literal::byte_string(field.name.as_bytes());

        // TODO remove ?
        let mut code = quote! { ::xmlib::de::DeserializeBuf::de_buf(&attr.value)? };
        if default.is_none() {
            code = quote! {::std::option::Option::Some(#code)};
        }
        let ident = &field.ident;
        attr_ser_code.push(quote! {
            #name_str => #ident = #code,
        });
    }

    for field in ty_value {
        let default = process_field(&field);
        let ty = field.ty;
        let ident = &field.ident;
        // TODO remove ?
        let mut code = quote! { ::xmlib::de::DeserializeElement::de(&mut reader__, e)? };
        let code = if field.has_multiple {
            quote! { #ident.push(#code) }
        } else {
            if default.is_none() {
                code = quote! {::std::option::Option::Some(#code)};
            }
            quote! { #ident = #code }
        };
        value_ser_code.push(quote! {
            Event::Start(e) if e.local_name() == <#ty as ::xmlib::de::DeserializeElement<R>>::name() => {
                #code;
            }
        });
    }

    if let Some(field) = ty_value_buf {
        let default = process_field(&field);
        // TODO remove ?
        let mut code = quote! { ::xmlib::de::DeserializeBuf::de_buf(e.into_inner().as_ref())? };
        if default.is_none() {
            code = quote! {::std::option::Option::Some(#code)};
        }
        let ident = &field.ident;
        let code = quote! { #ident = #code };

        // TODO find a better solution
        value_ser_code.push(quote! {
            Event::Text(e) if e.iter().all(|c| c.is_ascii_whitespace()) => {}
        });

        value_ser_code.push(quote! {
            Event::Text(e) => #code,
            /*Event::Start(e) if e.local_name() == <#ty as ::xmlib::de::Deserialize<R>>::name() => {
                // read the inner text
                #ident = match reader.read_event(&mut buf) {
                    Ok(Event::Text(e)) => ::xmlib::de::DeserializeBuf::de_buf(e.into_inner().as_ref())?,
                    Ok(Event::End(ref e))
                        if e.name() == <#ty as ::xmlib::de::Deserialize<R>>::name() => {
                            ::xmlib::de::DeserializeBuf::de_buf(&[])?
                    }
                    Err(e) => return Err(::xmlib::de::DeserError::XmlError(e)),
                    Ok(Event::Eof) => return Err(::xmlib::de::DeserError::XmlError(
                                xmlib::exports::Error::UnexpectedEof("Text".to_string())
                    )),
                    _ => return Err(::xmlib::de::DeserError::XmlError(
                            xmlib::exports::Error::TextNotFound)
                    ),
                };
                reader.read_to_end(<#ty as ::xmlib::de::Deserialize<R>>::name(), &mut buf)?;
            }*/
        });
    }

    if let Some(ident) = ty_collect_namespaces {
        init_code.push(quote! {let mut #ident = ::std::vec::Vec::new();});
        attr_ser_code.push(quote! {
            name => if name.len() >= 5 && &name[..5] == &b"xmlns"[..] {
                #ident.push((name.to_owned(), attr.value.into_owned()));
            }
        });
        finish_code.push(quote! {#ident, });
    }

    let raw_ser_name = proc_macro2::Literal::byte_string(raw_ser_name.as_bytes());

    // TODO documentation
    // TODO remove ?
    quote! {
        #[automatically_derived]
        impl<R: ::std::io::BufRead> ::xmlib::de::DeserializeElement<R> for #struct_ident {
            #[inline]
            fn name() -> &'static [u8] {
                #raw_ser_name
            }

            #[inline]
            fn de(
                mut reader__: &mut ::xmlib::de::XmlReader<R>,
                start__: ::xmlib::exports::events::BytesStart,
            ) -> ::std::result::Result<Self, ::xmlib::de::Error> {
                use ::xmlib::exports::events::Event;

                #(#init_code)*

                for attr in start__.attributes() {
                    let attr = match attr {
                        Ok(attr) => attr,
                        Err(e) => return ::std::result::Result::Err(::xmlib::de::Error {
                            ty_name: ::std::string::String::from_utf8_lossy(<#struct_ident as ::xmlib::de::DeserializeElement<R>>::name()).to_string(),
                            kind: ::xmlib::de::ErrorKind::XmlError(::xmlib::exports::Error::InvalidAttr(e)),
                        })
                    };
                    match attr.key {
                        #(#attr_ser_code)*
                        name => {
                            if let ::std::option::Option::Some(i) = ::xmlib::exports::memchr(b':', name) {
                                 println!("ignored attribute with namespace {} for {} (name = {})",
                                     ::std::string::String::from_utf8_lossy(&name[..i]),
                                     ::std::string::String::from_utf8_lossy(<#struct_ident as ::xmlib::de::DeserializeElement<R>>::name()),
                                     ::std::string::String::from_utf8_lossy(name)
                                );
                            } else {
                                return ::std::result::Result::Err(::xmlib::de::Error {
                                    ty_name: ::std::string::String::from_utf8_lossy(name).to_string(),
                                    kind: ::xmlib::de::ErrorKind::UnexpectedEvent(
                                        ::std::string::String::from_utf8_lossy(<#struct_ident as ::xmlib::de::DeserializeElement<R>>::name()).to_string(),
                                    ),
                                    }
                                )
                            }
                        }
                    }
                }

                let mut buf = ::std::vec::Vec::with_capacity(64);

                loop {
                    match reader__.read_event(&mut buf).unwrap() {
                        #(#value_ser_code)*
                        Event::End(e) if e.local_name() == #raw_ser_name => {
                            break;
                        }
                        Event::Text(e) if e.is_empty() => {}
                        // TODO
                        Event::Start(bytes) => {
                            let name = bytes.name();
                            if let ::std::option::Option::Some(i) = ::xmlib::exports::memchr(b':', name) {
                                println!("ignored namespaced element {} for {} (name = {})",
                                     ::std::string::String::from_utf8_lossy(&name[..i]),
                                     ::std::string::String::from_utf8_lossy(<#struct_ident as ::xmlib::de::DeserializeElement<R>>::name()),
                                     ::std::string::String::from_utf8_lossy(name)
                                );
                                if let Err(e) = reader__.read_to_end(name, &mut ::std::vec::Vec::with_capacity(64)) {
                                    return ::std::result::Result::Err(::xmlib::de::Error {
                                        ty_name: ::std::string::String::from_utf8_lossy(<#struct_ident as ::xmlib::de::DeserializeElement<R>>::name()).to_string(),
                                        kind: ::xmlib::de::ErrorKind::XmlError(e)
                                    })
                                }
                            } else {
                                return ::std::result::Result::Err(::xmlib::de::Error {
                                    ty_name: ::std::string::String::from_utf8_lossy(<#struct_ident as ::xmlib::de::DeserializeElement<R>>::name()).to_string(),
                                    kind: ::xmlib::de::ErrorKind::UnexpectedEvent(format!("start of {}",
                                        ::std::string::String::from_utf8_lossy(name),
                                    ))
                                })
                            }
                        }
                        e => {
                            return ::std::result::Result::Err(::xmlib::de::Error {
                                ty_name: ::std::string::String::from_utf8_lossy(<#struct_ident as ::xmlib::de::DeserializeElement<R>>::name()).to_string(),
                                kind: ::xmlib::de::ErrorKind::UnexpectedEvent(format!("{:?}", e))
                            })
                        }
                    }
                }

                #(#pre_finish_code)*
                #(#validation_code)*

                Ok(Self {
                    #(#finish_code)*
                })
            }
        }
    }
    .into()
}

fn create_validation(
    validation: &syn::Lit,
    ident: &Ident,
    ty_name: &String,
) -> proc_macro2::TokenStream {
    match validation {
        syn::Lit::Str(lit) => {
            let validation: proc_macro2::TokenStream =
                syn::parse2(syn::parse_str(&lit.value()).unwrap()).unwrap();
            quote! {
                if let ::std::result::Result::Err(e) = #validation(&#ident) {
                    return ::std::result::Result::Err(
                        ::xmlib::de::Error {
                            ty_name: ::std::string::String::from(#ty_name),
                            kind: ::xmlib::de::ErrorKind::Validation(format!("{:?}", e)),
                        }
                    );
                }
            }
        }
        lit => error!(
            validation.span(),
            format!("expected literal string but got {}", lit.to_token_stream())
        )
        .into(),
    }
}
