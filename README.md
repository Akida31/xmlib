# Xmlib
Convert rust datastructures from and into xml.

This library uses [quick_xml](https://github.com/tafia/quick-xml/) under the hood.


# Example
```rust
use xmlib_derive::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct Rectangle {
    width: u32,
    height: u32,
}

let rect = Rectangle { width: 13, height: 42 };

let serialized = xmlib::ser::write_to_string(rect).unwrap();
assert_eq!(serialized, r#"<rectangle width="13" height="42"/>"#);

let deserialized: Rectangle = xmlib::de::from_str(&serialized).unwrap();

assert_eq!(deserialized.width, 13);
assert_eq!(deserialized.height, 42);
```


# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in xmlib by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
