use heck::ToLowerCamelCase;
use proc_macro::TokenStream;
use syn::{spanned::Spanned, Data, Fields, Ident};

pub(crate) fn parse_input(input: TokenStream) -> Result<Input, TokenStream> {
    let input = match syn::parse_macro_input::parse::<syn::DeriveInput>(input) {
        Ok(data) => data,
        Err(err) => {
            return Err(TokenStream::from(err.to_compile_error()));
        }
    };

    let data = match input.data {
        Data::Struct(ref s) => match &s.fields {
            Fields::Named(fields) => {
                let raw_ser_name = match get_attr(&input.attrs, "rename")? {
                    AttrResult::Lit(lit) => get_literal_str(lit)?,
                    AttrResult::NotFound => input.ident.to_string().to_lower_camel_case(),
                    _ => return Err(error!(input.span(), "expected one single literal str")),
                };

                let no_constructor = match get_attr(&input.attrs, "no_constructor")? {
                    AttrResult::Existing => true,
                    AttrResult::NotFound => false,
                    _ => return Err(error!(input.span(), "expected \"no_constructor\"")),
                };
                InputData::NamedStruct(NamedStruct::parse(fields, no_constructor, raw_ser_name)?)
            }
            Fields::Unnamed(fields) => InputData::UnnamedStruct(UnnamedStruct::parse(fields)?),
            Fields::Unit => {
                return Err(error!(
                    input.span(),
                    "unit structs are not supported because they carry no data",
                ))
            }
        },
        Data::Enum(ref e) => InputData::Enum(Enum::parse(e)?),
        _ => {
            return Err(error!(
                input.span(),
                "can only be used for structs and enums"
            ))
        }
    };

    Ok(Input {
        data,
        ident: input.ident,
    })
}

pub(crate) struct Input {
    pub(crate) ident: syn::Ident,
    pub(crate) data: InputData,
}

pub(crate) enum InputData {
    Enum(Enum),
    NamedStruct(NamedStruct),
    UnnamedStruct(UnnamedStruct),
}

pub(crate) struct Enum {
    pub(crate) has_data: bool,
    pub(crate) variants: Vec<(Ident, proc_macro2::Literal, Option<syn::Type>)>,
}

impl Enum {
    fn parse(input: &syn::DataEnum) -> Result<Self, TokenStream> {
        // is not decided yet
        let mut has_data = None;

        let variants: Vec<_> = input
            .variants
            .iter()
            .map(|variant| {
                let name = match get_attr(&variant.attrs, "rename")? {
                    AttrResult::Lit(lit) => {
                        if has_data == Some(true) {
                            error!(ret: variant.span(), "rename has no effect for enums with data");
                        }
                        get_literal_str(lit)?
                    }
                    AttrResult::NotFound => variant.ident.to_string().to_lower_camel_case(),
                    _ => error!(ret: variant.span(), "expected one single literal str"),
                };

                let ty = match &variant.fields {
                    Fields::Unit => {
                        if has_data == Some(true) {
                            error!(ret: variant.span(),
                            "enums can be either with or without data but not both.");
                        }
                        has_data = Some(false);
                        None
                    }
                    Fields::Unnamed(fields) => {
                        if has_data == Some(false) {
                            error!(ret: variant.span(),
                            "enums can be either with or without values but not both.");
                        }
                        has_data = Some(true);
                        if fields.unnamed.len() != 1 {
                            error!(ret: fields.span(), format!(
                                "unnamed variants may have only one field but got {}",
                                fields.unnamed.len()
                            ));
                        }
                        Some(fields.unnamed[0].ty.clone())
                    }
                    _ => {
                        error!(ret: variant.span(), format!(
                            "only unit variants or unnamed variants are supported but got {:?}",
                            variant.ident
                        ))
                    }
                };

                let ident = variant.ident.clone();
                let name = proc_macro2::Literal::byte_string(name.as_bytes());

                Ok((ident, name, ty))
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            has_data: has_data.unwrap(),
            variants,
        })
    }
}

pub(crate) struct Field {
    pub(crate) ident: syn::Ident,
    pub(crate) name: String,
    pub(crate) default: Option<syn::Lit>,
    pub(crate) ty: syn::Type,
    pub(crate) has_multiple: bool,
    pub(crate) validation: Option<syn::Lit>,
}

pub(crate) struct NamedStruct {
    pub(crate) no_constructor: bool,
    pub(crate) raw_ser_name: String,
    pub(crate) ty_attribute: Vec<Field>,
    pub(crate) ty_value: Vec<Field>,
    pub(crate) ty_value_buf: Option<Field>,
    pub(crate) ty_collect_namespaces: Option<Ident>,
}

impl NamedStruct {
    fn parse(
        fields: &syn::FieldsNamed,
        no_constructor: bool,
        raw_ser_name: String,
    ) -> Result<Self, TokenStream> {
        let mut ty_attribute = Vec::new();
        let mut ty_value = Vec::new();
        let mut ty_value_buf = None;
        let mut ty_collect_namespaces = None;

        for field in &fields.named {
            let name = match get_attr(&field.attrs, "rename")? {
                AttrResult::Lit(lit) => get_literal_str(lit)?,
                AttrResult::NotFound => field
                    .ident
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .to_lower_camel_case(),
                _ => error!(ret: field.span(), "expected one single literal str"),
            };

            let default_lit = syn::Lit::Str(syn::LitStr::new(
                "::std::default::Default::default()",
                field.span(),
            ));

            let val_ty = get_val_ty(field)?;

            let default = match get_attr(&field.attrs, "default")? {
                AttrResult::Lit(default) => {
                    if val_ty == ValueTy::CollectNamespaces {
                        error!(ret: field.span(),
                            "\"default\" can't be combined with \"collect_namespaces\"",
                        );
                    } else {
                        Some(default)
                    }
                }
                AttrResult::NotFound => None,
                AttrResult::Multiple => {
                    error!(ret: field.span(),
                        "multiple attribute values found for \"default\"",
                    );
                }
                AttrResult::Existing => Some(default_lit),
            };

            let has_multiple = match get_attr(&field.attrs, "multiple")? {
                AttrResult::Existing => {
                    if val_ty != ValueTy::Value {
                        error!(ret: fields.span(), "multiple can only used with value");
                    }
                    true
                }
                AttrResult::NotFound => false,
                AttrResult::Multiple => {
                    error!(ret: field.span(),
                        "multiple attribute values found for \"multiple\"",
                    );
                }
                AttrResult::Lit(_) => {
                    error!(ret: field.span(), "expected multiple");
                }
            };

            let validation = match get_attr(&field.attrs, "validate")? {
                AttrResult::Lit(lit) => Some(lit),
                AttrResult::NotFound => None,
                _ => error!(ret: field.span(), "expected one single literal str for validate"),
            };

            let constructed_field = Field {
                ident: field.ident.clone().unwrap(),
                name,
                default,
                ty: field.ty.clone(),
                has_multiple,
                validation,
            };

            match val_ty {
                ValueTy::Attr => ty_attribute.push(constructed_field),
                ValueTy::Value => ty_value.push(constructed_field),
                ValueTy::ValueBuf => {
                    if ty_value_buf.is_some() {
                        error!(ret: field.span(),
                            "only one attribute may be annotated with \"value_buf\""
                        )
                    }
                    ty_value_buf = Some(constructed_field);
                }
                ValueTy::CollectNamespaces => {
                    if ty_collect_namespaces.is_some() {
                        error!(ret: field.span(),
                            "only one attribute may be annotated with \"collect_namespaces\""
                        )
                    }
                    ty_collect_namespaces = Some(constructed_field.ident);
                }
            }
        }

        Ok(Self {
            no_constructor,
            raw_ser_name,
            ty_attribute,
            ty_value,
            ty_value_buf,
            ty_collect_namespaces,
        })
    }
}

pub(crate) struct UnnamedStruct {
    pub(crate) validation: Option<syn::Lit>,
    pub(crate) ty: syn::Type,
}

impl UnnamedStruct {
    fn parse(fields: &syn::FieldsUnnamed) -> Result<Self, TokenStream> {
        if fields.unnamed.len() != 1 {
            Err(error!(
                fields.span(),
                "only unnamed structs with one field are supported"
            ))
        } else {
            let field = &fields.unnamed[0];
            let validation = match get_attr(&field.attrs, "validate")? {
                AttrResult::Lit(lit) => Some(lit),
                AttrResult::NotFound => None,
                _ => error!(ret: field.span(), "expected one single literal str for validate"),
            };

            Ok(Self {
                validation,
                ty: field.ty.clone(),
            })
        }
    }
}

fn get_literal_str(lit: syn::Lit) -> Result<String, TokenStream> {
    if let syn::Lit::Str(ref s) = lit {
        Ok(s.value())
    } else {
        Err(error!(
            lit.span(),
            format!("expected literal str, got {:?}", lit)
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueTy {
    Attr,
    Value,
    ValueBuf,
    CollectNamespaces,
}

fn get_val_ty(field: &syn::Field) -> Result<ValueTy, TokenStream> {
    use AttrResult::{Existing, Lit, Multiple, NotFound};

    Ok(
        match (
            get_attr(&field.attrs, "value")?,
            get_attr(&field.attrs, "value_buf")?,
            get_attr(&field.attrs, "collect_namespaces")?,
        ) {
            (NotFound, NotFound, NotFound) => ValueTy::Attr,

            (Existing, NotFound, NotFound) => ValueTy::Value,
            (NotFound, Existing, NotFound) => ValueTy::ValueBuf,
            (NotFound, NotFound, Existing) => ValueTy::CollectNamespaces,

            (NotFound, Existing, Existing)
            | (Existing, NotFound, Existing)
            | (Existing, Existing, NotFound)
            | (Existing, Existing, Existing) => {
                return Err(error!(
                    field.span(),
                    "\"value\", \"value_buf\" and \"collect_namespaces\" can not be combined.",
                ));
            }

            (Multiple, _, _) => {
                return Err(error!(
                    field.span(),
                    "multiple attribute values found for \"value\"",
                ));
            }
            (_, Multiple, _) => {
                return Err(error!(
                    field.span(),
                    "multiple attribute values found for \"value_buf\"",
                ));
            }
            (_, _, Multiple) => {
                return Err(error!(
                    field.span(),
                    "multiple attribute values found for \"collect_namespaces\"",
                ));
            }

            (Lit(_), _, _) | (_, Lit(_), _) | (_, _, Lit(_)) => {
                return Err(error!(field.span(), "expected value"));
            }
        },
    )
}

#[derive(Debug, PartialEq)]
enum AttrResult {
    NotFound,
    Multiple,
    Lit(syn::Lit),
    Existing,
}

fn get_attr(attrs: &[syn::Attribute], name: &str) -> Result<AttrResult, TokenStream> {
    let mut res = AttrResult::NotFound;
    for attr in attrs {
        if !attr.path.is_ident("xmlib") {
            continue;
        }
        let meta = match attr.parse_meta() {
            Ok(meta) => meta,
            Err(e) => error!(ret: attr.span(), e),
        };
        match meta {
            syn::Meta::List(ref meta) => {
                for meta in &meta.nested {
                    let meta = match meta {
                        syn::NestedMeta::Meta(meta) => meta,
                        syn::NestedMeta::Lit(_) => {
                            // TODO is this correct?
                            continue;
                        }
                    };
                    match meta {
                        syn::Meta::NameValue(ref meta) => {
                            if meta.path.is_ident(name) {
                                if res != AttrResult::NotFound {
                                    return Ok(AttrResult::Multiple);
                                }
                                res = AttrResult::Lit(meta.lit.clone());
                            }
                        }
                        syn::Meta::Path(ref path) => {
                            if path.is_ident(name) {
                                if res != AttrResult::NotFound {
                                    return Ok(AttrResult::Multiple);
                                }
                                res = AttrResult::Existing;
                            }
                        }
                        a => todo!("{:?}", a),
                    }
                }
            }
            _ => panic!("expected ser(...)"),
        }
    }
    Ok(res)
}
