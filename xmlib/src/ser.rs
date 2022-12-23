//! Serialize rust datastructures into XML data.

use std::io::{self, Write};

/// Serializes the value to a string.
///
/// # Example
/// ```
/// use xmlib_derive::Serialize;
///
/// #[derive(Serialize)]
/// struct Rectangle {
///     width: u32,
///     height: u32,
/// }
///
/// let rect = Rectangle { width: 13, height: 42 };
///
/// let serialized = xmlib::ser::write_to_string(rect).unwrap();
/// assert_eq!(serialized, r#"<rectangle width="13" height="42"/>"#);
/// ```
pub fn write_to_string<T: Serialize<Vec<u8>>>(value: T) -> io::Result<String> {
    let mut writer = XmlWriter::new(Vec::with_capacity(128))?;
    value.ser(&mut writer)?;
    String::from_utf8(writer.into_inner())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Interface for writing XML values
pub struct XmlWriter<W: Write> {
    writer: W,
}

impl<W: Write> XmlWriter<W> {
    /// Creates a new [`XmlWriter`]
    pub fn new(writer: W) -> io::Result<Self> {
        let s = Self { writer };
        // TODO
        //s.write_xml_start()?;
        Ok(s)
    }

    /// Writes the start of a xml file
    ///
    /// `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>`
    pub fn write_xml_start(&mut self) -> io::Result<()> {
        self.writer
            .write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#)
    }

    /// Consumes the `XmlWriter`, returning the wrapped writer.
    #[inline]
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> std::ops::Deref for XmlWriter<W> {
    type Target = W;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl<W: Write> std::ops::DerefMut for XmlWriter<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

/// Serialize a XML Element to a [`XmlWriter`]
pub trait Serialize<W: Write> {
    /// Serialization function
    ///
    /// Mark this as `#[inline]`
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()>;
}

macro_rules! impl_ser_num {
    ($t:ty) => {
        impl<W: Write> Serialize<W> for $t {
            #[inline]
            fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
                let mut buffer = itoa::Buffer::new();
                let s = buffer.format(*self);
                writer.write_all(s.as_bytes())
            }
        }
    };
    ($($t:ty),+$(,)?) => {
        $(impl_ser_num!($t);)+
    }
}

macro_rules! impl_ser_float {
    ($t:ty) => {
        impl<W: Write> Serialize<W> for $t {
            #[inline]
            fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
                let mut buffer = ryu::Buffer::new();
                let s = buffer.format(*self);
                writer.write_all(s.as_bytes())
            }
        }
    };
    ($($t:ty),+$(,)?) => {
        $(impl_ser_float!($t);)+
    }
}

impl_ser_num!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);
impl_ser_float!(f32, f64);

impl<W: Write, T> Serialize<W> for &T
where
    T: Serialize<W>,
{
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        T::ser(self, writer)
    }
}

impl<W: Write, T> Serialize<W> for &mut T
where
    T: Serialize<W>,
{
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        T::ser(self, writer)
    }
}

impl<W: Write> Serialize<W> for &str {
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        writer.write_all(self.as_bytes())
    }
}

impl<W: Write> Serialize<W> for String {
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        writer.write_all(self.as_bytes())
    }
}

impl<W: Write> Serialize<W> for bool {
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        writer.write_all(match self {
            true => b"1",
            false => b"0",
        })
    }
}

impl<W: Write, T> Serialize<W> for Option<T>
where
    T: Serialize<W>,
{
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        match self {
            Some(val) => val.ser(writer),
            None => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cannot serialize None",
            )),
        }
    }
}

impl<W: Write, T> Serialize<W> for &[T]
where
    T: Serialize<W>,
{
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        for val in *self {
            match val.ser(writer) {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl<W: Write, T> Serialize<W> for Vec<T>
where
    T: Serialize<W>,
{
    #[inline]
    fn ser(&self, writer: &mut XmlWriter<W>) -> io::Result<()> {
        for val in self {
            match val.ser(writer) {
                Ok(()) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
