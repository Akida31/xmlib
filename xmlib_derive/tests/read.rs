use xmlib_derive::Deserialize;

use xmlib::de::{CollectNamespaces, DeserializeBuf, DeserializeElement};

#[derive(Debug, Deserialize, PartialEq)]
pub enum SomeInnerEnum {
    A,
    B,
}

#[test]
fn deser_enum() {
    assert_eq!(SomeInnerEnum::A, SomeInnerEnum::de_buf(&b"a"[..]).unwrap());
    assert_eq!(SomeInnerEnum::B, SomeInnerEnum::de_buf(&b"b"[..]).unwrap());
    assert!(SomeInnerEnum::de_buf(&b"A"[..]).is_err());
    assert!(SomeInnerEnum::de_buf(&b"ab"[..]).is_err());
    assert!(SomeInnerEnum::de_buf(&b"e"[..]).is_err());
}

#[test]
fn deser_newtype() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct NewType(SomeInnerEnum);

    assert_eq!(
        NewType(SomeInnerEnum::A),
        NewType::de_buf(&b"a"[..]).unwrap()
    );
    assert_eq!(
        NewType(SomeInnerEnum::B),
        NewType::de_buf(&b"b"[..]).unwrap()
    );
    assert!(NewType::de_buf(&b"A"[..]).is_err());
    assert!(NewType::de_buf(&b"ab"[..]).is_err());
    assert!(NewType::de_buf(&b"e"[..]).is_err());
}

fn read_struct<'a, T: DeserializeElement<std::io::BufReader<&'a [u8]>>>(
    input: &'a [u8],
) -> Option<T> {
    let mut reader = xmlib::de::XmlReader::new(std::io::BufReader::new(input));

    use xmlib::exports::events::Event;
    let mut buf = Vec::with_capacity(32);
    let mut s = None;
    loop {
        match reader.read_event(&mut buf).unwrap() {
            Event::Start(e) if e.local_name() == b"struct" => {
                s = Some(T::de(&mut reader, e).unwrap());
            }
            Event::Eof if s.is_some() => {
                break;
            }
            Event::Text(e) if e.len() == 0 => {}
            e => unreachable!("{:?}", e),
        }
    }
    s
}

#[test]
fn read() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Struct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(default = "123")]
        b: String,
        c: String,
        #[xmlib(default = "SomeInnerEnum::B")]
        eff: SomeInnerEnum,
    }

    let input: Vec<u8> = br#"<struct a="1" c="Hi" />"#.to_vec();
    let s = read_struct(&input);
    assert_eq!(
        s,
        Some(Struct {
            a: 1,
            c: String::from("Hi"),
            b: String::from("123"),
            eff: SomeInnerEnum::B,
        })
    );
}

#[test]
fn option() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Struct {
        #[xmlib(default = "None")]
        a: Option<u8>,
        b: Option<String>,
    }

    let input: Vec<u8> = br#"<struct b="Hi" />"#.to_vec();
    let s = read_struct(&input);
    assert_eq!(
        s,
        Some(Struct {
            a: None,
            b: Some(String::from("Hi")),
        })
    );
}

#[test]
fn value() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Struct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(value)]
        i: InnerStruct,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct InnerStruct {
        a: u8,
        #[xmlib(default = 0)]
        b: i16,
    }

    let input: Vec<u8> = br#"<struct a="1"><innerStruct a="2"/></struct>"#.to_vec();
    let s = read_struct(&input);
    assert_eq!(
        s,
        Some(Struct {
            a: 1,
            i: InnerStruct { a: 2, b: 0 }
        })
    );
}

#[test]
fn multiple() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Struct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(value, multiple)]
        i: Vec<InnerStruct>,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct InnerStruct {
        a: u8,
        #[xmlib(default = 0)]
        b: i16,
    }

    let input: Vec<u8> =
        br#"<struct a="1"><innerStruct a="2"/><innerStruct a="5" b="2"/></struct>"#.to_vec();
    let s = read_struct(&input);
    assert_eq!(
        s,
        Some(Struct {
            a: 1,
            i: vec![InnerStruct { a: 2, b: 0 }, InnerStruct { a: 5, b: 2 }]
        })
    );
}

#[test]
fn different_values() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Struct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(value)]
        i: InnerStruct,
        #[xmlib(value)]
        j: InnerStruct2,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct InnerStruct {
        a: u8,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct InnerStruct2 {
        a: u8,
    }

    let input: Vec<u8> =
        br#"<struct a="1"><innerStruct a="2"/><innerStruct2 a="5"/></struct>"#.to_vec();
    let s = read_struct(&input);
    assert_eq!(
        s,
        Some(Struct {
            a: 1,
            i: InnerStruct { a: 2 },
            j: InnerStruct2 { a: 5 }
        })
    );
}

#[test]
fn value_buf() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Struct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(value)]
        i: InnerStruct,
        #[xmlib(value_buf)]
        b: u32,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct InnerStruct {
        a: u8,
        #[xmlib(value_buf)]
        b: String,
    }

    let input: Vec<u8> =
        br#"<struct a="1">12<innerStruct a="2">Hey!</innerStruct></struct>"#.to_vec();
    let s = read_struct(&input);
    assert_eq!(
        s,
        Some(Struct {
            a: 1,
            i: InnerStruct {
                a: 2,
                b: String::from("Hey!")
            },
            b: 12,
        })
    );
}

#[test]
fn namespaces() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Struct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(value)]
        i: InnerStruct,
        #[xmlib(collect_namespaces)]
        namespaces: CollectNamespaces,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct InnerStruct {
        b: u8,
        #[xmlib(collect_namespaces)]
        namespaces: CollectNamespaces,
    }

    let input: Vec<u8> =
        br#"<struct xmlns:r="hi" xmlns="hey" a="1"><innerStruct xmlns:r="hi" b="42"/></struct>"#
            .to_vec();
    let s: Struct = read_struct(&input).unwrap();
    assert_eq!(s.a, 1);
    assert_eq!(s.i.b, 42);
}
