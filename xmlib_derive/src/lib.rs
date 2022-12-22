//! This crate provides two derive macros:
//!
//! ```
//! # use xmlib_derive::{Serialize, Deserialize};
//! #
//! #[derive(Serialize, Deserialize)]
//! # struct S(i32);
//! #
//! # fn main() {}
//! ```
//! # Examples
//! ## Simple Struct
//! ```
//! use xmlib_derive::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! struct Rectangle {
//!     width: u32,
//!     height: u32,
//! }
//!
//! let rect = Rectangle { width: 13, height: 42 };
//!
//! let serialized = xmlib::ser::write_to_string(rect).unwrap();
//! assert_eq!(serialized, r#"<rectangle width="13" height="42"/>"#);
//!
//! let deserialized: Rectangle = xmlib::de::from_str(&serialized).unwrap();
//!
//! assert_eq!(deserialized.width, 13);
//! assert_eq!(deserialized.height, 42);
//! ```
//!
//! ## Renamed struct and attributes
//! ```
//! use xmlib_derive::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! #[xmlib(rename = "square")]
//! struct Rectangle {
//!     #[xmlib(rename = "size")]
//!     width: u32,
//! }
//!
//! let rect = Rectangle { width: 42 };
//!
//! let serialized = xmlib::ser::write_to_string(rect).unwrap();
//! assert_eq!(serialized, r#"<square size="42"/>"#);
//!
//! let deserialized: Rectangle = xmlib::de::from_str(&serialized).unwrap();
//!
//! assert_eq!(deserialized.width, 42);
//! ```
//!
//! ## Validation
//! ```
//! use xmlib_derive::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug)]
//! struct Square {
//!     #[xmlib(validate = "is_positive")]
//!     size: i32,
//! }
//!
//! fn is_positive(size: &i32) -> Result<(), i32> {
//!     if *size > 0 {
//!         Ok(())
//!     } else {
//!         Err(*size)
//!     }
//! }
//!
//! let valid = r#"<square size="42"/>"#;
//! let valid: Result<Square, _> = xmlib::de::from_str(&valid);
//! assert!(valid.is_ok());
//!
//! let invalid = r#"<square size="-1"/>"#;
//! let invalid: Result<Square, _> = xmlib::de::from_str(&invalid);
//! assert!(invalid.is_err());
//! ```
//!
//! ## Nested structs
//! ```
//! use xmlib_derive::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
//! struct Inner {
//!     #[xmlib(default = 42)]
//!     foo: u32,
//!     bar: String,
//! }
//!
//! #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
//! struct Outer {
//!     #[xmlib(value_buf)]
//!     baz: bool,
//!     #[xmlib(value)]
//!     inner: Inner,
//! }
//!
//! let outer = Outer { baz: false, inner: Inner { foo: 42, bar: String::from("13"), } };
//!
//! let serialized = xmlib::ser::write_to_string(&outer).unwrap();
//! assert_eq!(serialized, r#"<outer><inner bar="13"/>0</outer>"#);
//!
//! let deserialized: Outer = xmlib::de::from_str(&serialized).unwrap();
//!
//! assert_eq!(deserialized, outer);
//! ```
//!
//! # Structs
//!
//! ## Unnamed structs
//! Only newtype structs (unnamed structs with exactly one field) are supported.
//! They serialize/ deserialize only the inner value.
//!
//! See also [Validation](#validation)
//!
//! ## Named structs
//! All attribute names will be renamed to lower camel case.
//! Use `#[xmlib(rename = "name")]` to serialize and deserialize
//! the field with the given name instead of the rust name.
//!
//! Unless you attribute the struct with `#[xmlib(no_constructor)]` a public function
//! `with_default` will be generated to instantiate the struct.
//!
//! Each field will be an xml attribute by default, unless you specify it otherwise.
//!
//! If the field has it's default value it won't be serialized to shorten the text.
//!
//! If you want the field to be serialized and deserialized as a child instead of an attribute,
//! consider annotating the field with `#[xmlib(value)]`.
//!
//! In addition to `value` you can annotate a field with `#[xmlib(multiple)]` to allow multiple
//! children with the same name. Note that the type of the field must be [`std::vec::Vec`].
//!
//! `#[xmlib(value_buf)]` can be used to used to serialize/ deserialize the text content of the
//! element.
//!
//! `#[xmlib(collect_namespaces)]` can be used to serialize/ deserialize all `xmlns=""` attributes.
//!
//! You can annotate a field with `#[xmlib(default)]` or `#[xmlib(default = value)]` to use
//! [`Default::default()`] or `value` if the field is not present when deserializing.
//!
//! See also [Validation](#validation)
//!
//! # Enums
//! Currently either all variants must have data or all mustn't have data.
//!
//! ## Enums without Data:
//! All variants will be renamed to lower camel case.
//! Use `#[xmlib(rename = "name")]` to serialize and deserialize
//! the field with the given name instead of the rust name.
//!
//! ## Enums with data
//! When deserializing the first successfull variant will be chosen.
//!
//! # Validation
//! You can annotate struct fields with `#[xmlib(validate = "fn_name")]` to cause an error in the
//! deserialization. The function must take one single shared reference to the type of the field as
//! the argument and return `Result<(), Error>` where Error is any type implementing debug.
use proc_macro::TokenStream;

macro_rules! error {
    (ret: $span:expr, $msg:expr $(,)?) => {
        return Err(error!($span, $msg))
    };
    ($span:expr, $msg:expr $(,)?) => {
        TokenStream::from(syn::Error::new($span, $msg).to_compile_error())
    };
}

mod de;
mod parse;
mod ser;

/// Creates an implementation of [`xmlib::ser::Serialize`](../xmlib/ser/trait.Serialize.html).
///
/// See the [crate documentation][crate] for more details.
#[proc_macro_derive(Serialize, attributes(xmlib))]
pub fn expand_ser(input: TokenStream) -> TokenStream {
    match parse::parse_input(input) {
        Ok(input) => ser::expand(input),
        Err(e) => e,
    }
}

/// Creates an implementation of [`xmlib::de::DeserializeElement`](../xmlib/de/trait.DeserializeElement.html) for named structs
/// and an implementation of [`xmlib::de::DeserializeBuf`](../xmlib/de/trait.DeserializeBuf.html) for enums and unnamed structs.
///
/// See the [crate documentation][crate] for more details.
#[proc_macro_derive(Deserialize, attributes(xmlib))]
pub fn expand_de(input: TokenStream) -> TokenStream {
    match parse::parse_input(input) {
        Ok(input) => de::expand(input),
        Err(e) => e,
    }
}
