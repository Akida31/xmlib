use xmlib_derive::Serialize;

use xmlib::ser::Serialize;

#[derive(Debug, Serialize, PartialEq)]
pub enum SomeInnerEnum {
    A,
    B,
}

impl std::str::FromStr for SomeInnerEnum {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "A" => Ok(Self::A),
            "B" => Ok(Self::B),
            v => Err(v.to_owned()),
        }
    }
}

fn ser<T: Serialize<Vec<u8>>>(v: &T) -> Result<String, std::string::FromUtf8Error> {
    let mut writer = xmlib::ser::XmlWriter::new(Vec::with_capacity(128)).unwrap();
    v.ser(&mut writer).unwrap();
    String::from_utf8(writer.clone())
}

#[test]
fn ser_struct() {
    #[derive(Serialize, Debug, PartialEq)]
    struct Struct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(default = "123")]
        b: String,
        c: String,
        #[xmlib(default = "SomeInnerEnum::B")]
        eff: SomeInnerEnum,
    }

    let mut a = Struct::with_default(String::from("abc"));

    assert_eq!(ser(&a).unwrap(), r#"<struct c="abc"/>"#);

    a.a = 1;

    assert_eq!(ser(&a).unwrap(), r#"<struct a="1" c="abc"/>"#);
}

#[test]
fn renamed_struct() {
    #[derive(Serialize, Debug)]
    #[xmlib(rename = "newName")]
    struct RenamedStruct {
        #[xmlib(default = 0)]
        a: u8,
        #[xmlib(default = "123")]
        b: String,
        #[xmlib(default = 12, value)]
        c: u8,
        #[xmlib(value)]
        d: SubStruct,
    }

    #[derive(Serialize, Debug)]
    struct SubStruct {
        a: u8,
        b: String,
        should_be_renamed: u8,
    }

    let mut a = RenamedStruct::with_default(SubStruct {
        a: 2,
        b: String::from("bcd"),
        should_be_renamed: 1,
    });

    assert_eq!(
        ser(&a).unwrap(),
        r#"<newName><subStruct a="2" b="bcd" shouldBeRenamed="1"/></newName>"#
    );

    a.a = 1;
    a.c = 1;

    assert_eq!(
        ser(&a).unwrap(),
        r#"<newName a="1">1<subStruct a="2" b="bcd" shouldBeRenamed="1"/></newName>"#
    );
}

#[test]
fn ser_enum() {
    #[allow(dead_code)]
    #[derive(Serialize, Debug)]
    enum SomeEnum {
        HelloWorld,
        OtherName,
    }

    let a = SomeEnum::HelloWorld;

    assert_eq!(ser(&a).unwrap(), r#"helloWorld"#);
}

#[test]
fn ser_renamed_enum() {
    #[derive(Serialize, Debug)]
    enum RenamedEnum {
        HelloWorld,
        #[xmlib(rename = "veryOtherName")]
        OtherName,
        R1C1,
        X1024x768,
    }

    assert_eq!(ser(&RenamedEnum::HelloWorld).unwrap(), r#"helloWorld"#);
    assert_eq!(ser(&RenamedEnum::OtherName).unwrap(), r#"veryOtherName"#);
    assert_eq!(ser(&RenamedEnum::R1C1).unwrap(), r#"r1c1"#);
    assert_eq!(ser(&RenamedEnum::X1024x768).unwrap(), r#"x1024x768"#);
}

#[test]
fn unnamed_struct() {
    #[derive(Serialize, Debug)]
    struct Unnamed(u8);

    assert_eq!(ser(&Unnamed(42)).unwrap(), r#"42"#);
}

// TODO trybuild
/*fn invalid_enum() {
    #[derive(Serialize, Debug)]
    enum RenamedEnum {
        HelloWorld,
        A(u8),
    }
}*/
