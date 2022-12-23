//! # Xmlib
//! Convert rust datastructures from and into xml.
//!
//! This library uses [quick_xml](https://github.com/tafia/quick-xml/) under the hood.
//!
//!
//! # Example
//! ```rust
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

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::perf)]

pub mod de;
mod error;
pub mod ser;

pub use error::{Error, ErrorKind};

/// Exports of [`memchr::memchr`] and [`quick_xml`]
pub mod exports {
    pub use memchr::memchr;
    pub use quick_xml::*;
}
