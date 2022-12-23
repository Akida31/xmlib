//! Deserialize rust datastructures into XML data.

pub use crate::{Error, ErrorKind};

/// Wrapper for [`quick_xml::Reader`] but adds and specialized some methods to improve performance.
pub struct XmlReader<R: std::io::BufRead> {
    reader: quick_xml::Reader<R>,
}

impl<'a> XmlReader<std::io::BufReader<zip::read::ZipFile<'a>>> {
    /// Creates a new [`XmlReader`] from a [`zip::read::ZipFile`].
    ///
    /// The reading will be buffered through [`std::io::BufReader`]
    pub fn from_file(file: zip::read::ZipFile<'a>) -> Self {
        Self::new(std::io::BufReader::new(file))
    }
}

impl<R: std::io::BufRead> XmlReader<R> {
    /// Creates a new [`XmlReader`] from a [`std::io::BufRead`].
    ///
    /// To speed the reading up most checks from `quick_xml` are disabled.
    /// If you want to enable them, consider using [`XmlReader::from_xml_reader`].
    pub fn new(reader: R) -> Self {
        let mut reader = quick_xml::Reader::from_reader(reader);
        reader
            .check_end_names(false)
            .trim_text(false)
            .check_comments(false)
            .expand_empty_elements(true);
        Self::from_xml_reader(reader)
    }

    /// Creates a new [`XmlReader`] from a [`quick_xml::Reader`].
    ///
    /// Consider using [`XmlReader::new`] instead, if you don't want to customize the
    /// [`quick_xml::Reader`].
    pub fn from_xml_reader(reader: quick_xml::Reader<R>) -> Self {
        Self { reader }
    }

    /// Specialized version from [`quick_xml::Reader::read_text`] because it took around 24 % of
    /// total CPU time for a microbenchmark.
    ///
    /// See it's documentation for more information.
    pub fn read_text_bytes<'a, K: AsRef<[u8]>>(
        &mut self,
        end: K,
        buf: &'a mut Vec<u8>,
        other_buf: &mut Vec<u8>,
    ) -> quick_xml::Result<quick_xml::events::BytesText<'a>> {
        use quick_xml::events::Event;

        let s = match self.read_event(buf) {
            Ok(Event::Text(e)) => Ok(e),
            Ok(Event::End(ref e)) if e.name() == end.as_ref() => {
                return Ok(quick_xml::events::BytesText::from_escaped(&[][..]))
            }
            Err(e) => return Err(e),
            Ok(Event::Eof) => return Err(quick_xml::Error::UnexpectedEof("Text".to_string())),
            _ => return Err(quick_xml::Error::TextNotFound),
        };
        self.read_to_end(end, other_buf)?;
        s
    }

    /// Specialized version from [`quick_xml::Reader::read_text`]
    /// because it took noticable CPU time for a microbenchmark.
    ///
    /// See it's documentation for more information.
    pub fn read_text<K: AsRef<[u8]>>(
        &mut self,
        end: K,
        buf: &mut Vec<u8>,
        other_buf: &mut Vec<u8>,
    ) -> Result<String, ErrorKind> {
        let bytes = match self.read_text_bytes(end, buf, other_buf) {
            Ok(bytes) => bytes,
            Err(e) => return Err(ErrorKind::XmlError(e)),
        };
        let unescaped_bytes = match quick_xml::escape::unescape(&bytes) {
            Ok(bytes) => bytes,
            Err(e) => return Err(ErrorKind::XmlError(quick_xml::Error::EscapeError(e))),
        };

        String::from_utf8(unescaped_bytes.to_vec()).map_err(ErrorKind::FromUtf8Error)
    }
}

impl<R: std::io::BufRead> std::ops::Deref for XmlReader<R> {
    type Target = quick_xml::Reader<R>;

    fn deref(&self) -> &Self::Target {
        &self.reader
    }
}

impl<R: std::io::BufRead> std::ops::DerefMut for XmlReader<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reader
    }
}

/// Deserialize an element.
pub trait DeserializeElement<R: std::io::BufRead>
where
    Self: Sized,
{
    /// Gets the name of the element.
    ///
    /// This is used by parents to find the type for each child.
    /// The slice should be valid utf-8, but isn't currently required to do so.
    fn name() -> &'static [u8];

    /// Gets the name of the element as string.
    ///
    /// This performs an allocation and converts to the string lossily.
    #[inline]
    fn name_string() -> String {
        String::from_utf8_lossy(<Self as DeserializeElement<R>>::name()).to_string()
    }

    /// Deserializes the element from the reader.
    fn de(reader: &mut XmlReader<R>, start: quick_xml::events::BytesStart) -> Result<Self, Error>;
}

/// Deserialize an attribute/ text.
pub trait DeserializeBuf
where
    Self: Sized,
{
    /// Deserializes the value from the given bytes.
    fn de_buf(buf: &[u8]) -> Result<Self, Error>;
}

impl<R: std::io::BufRead, T> DeserializeElement<R> for Vec<T>
where
    T: DeserializeElement<R>,
{
    #[inline]
    fn name() -> &'static [u8] {
        T::name()
    }

    #[inline]
    fn de(
        _reader: &mut XmlReader<R>,
        _start: quick_xml::events::BytesStart,
    ) -> Result<Self, Error> {
        Err(Error {
            ty_name: format!("Vec<{}>", String::from_utf8_lossy(T::name())),
            kind: ErrorKind::InvalidType(String::from(
                "Cannot deserialize Vec. Use the `multiple` attribute",
            )),
        })
    }
}

impl<R: std::io::BufRead, T> DeserializeElement<R> for Option<T>
where
    T: DeserializeElement<R>,
{
    #[inline]
    fn name() -> &'static [u8] {
        T::name()
    }

    #[inline]
    fn de(reader: &mut XmlReader<R>, start: quick_xml::events::BytesStart) -> Result<Self, Error> {
        T::de(reader, start).map(Some)
    }
}

impl<R: std::io::BufRead, T> DeserializeElement<R> for Box<T>
where
    T: DeserializeElement<R>,
{
    #[inline]
    fn name() -> &'static [u8] {
        T::name()
    }

    #[inline]
    fn de(reader: &mut XmlReader<R>, start: quick_xml::events::BytesStart) -> Result<Self, Error> {
        T::de(reader, start).map(Box::new)
    }
}

impl DeserializeBuf for String {
    #[inline]
    fn de_buf(buf: &[u8]) -> Result<Self, Error> {
        Self::from_utf8(buf.to_vec()).map_err(|e| Error {
            ty_name: String::from("String"),
            kind: ErrorKind::FromUtf8Error(e),
        })
    }
}

impl<T> DeserializeBuf for Option<T>
where
    T: DeserializeBuf,
{
    #[inline]
    fn de_buf(buf: &[u8]) -> Result<Self, Error> {
        T::de_buf(buf).map(Some)
    }
}

impl DeserializeBuf for bool {
    #[inline]
    fn de_buf(buf: &[u8]) -> Result<Self, Error> {
        match buf {
            b"0" | b"false" => Ok(false),
            b"1" | b"true" => Ok(true),
            v => Err(Error {
                ty_name: String::from("bool"),
                kind: ErrorKind::InvalidType(String::from_utf8_lossy(v).to_string()),
            }),
        }
    }
}

impl<T> DeserializeBuf for Box<T>
where
    T: DeserializeBuf,
{
    #[inline]
    fn de_buf(buf: &[u8]) -> Result<Self, Error> {
        T::de_buf(buf).map(Box::new)
    }
}

macro_rules! impl_de_num_signed {
    ($t:ty) => {
        impl DeserializeBuf for $t {
            #[inline]
            fn de_buf(buf: &[u8]) -> Result<Self, Error> {
                let (s, read) = atoi::FromRadix10Signed::from_radix_10_signed(buf);

                if read != buf.len() {
                    Err(Error {
                        ty_name: String::from(stringify!($t)),
                        kind: ErrorKind::InvalidType(format!("read only {} of {} bytes in {}",
                                                             read, buf.len(), String::from_utf8_lossy(buf))),
                    })
                } else {
                    Ok(s)
                }
            }
        }
    };
    ($($t:ty),+$(,)?) => {
        $(impl_de_num_signed!($t);)+
    }
}

macro_rules! impl_de_num_unsigned {
    ($t:ty) => {
        impl DeserializeBuf for $t {
            #[inline]
            fn de_buf(buf: &[u8]) -> Result<Self, Error> {
                let (s, read) = atoi::FromRadix10::from_radix_10(&buf);

                if read != buf.len() {
                    Err(Error {
                        ty_name: String::from(stringify!($t)),
                        kind: ErrorKind::InvalidType(format!("read only {} of {} bytes in {}",
                                                             read, buf.len(), String::from_utf8_lossy(buf))),
                    })
                } else {
                    Ok(s)
                }
            }
        }
    };
    ($($t:ty),+$(,)?) => {
        $(impl_de_num_unsigned!($t);)+
    }
}

macro_rules! impl_de_float {
    ($t:ty) => {
        impl DeserializeBuf for $t {
            #[inline]
            fn de_buf(buf: &[u8]) -> Result<Self, Error> {
                fast_float::parse(buf).map_err(|_| Error {
                    ty_name: String::from(stringify!($t)),
                    kind: ErrorKind::InvalidType(
                        String::from_utf8_lossy(buf).to_string(),
                    ),
                })
            }
        }
    };
    ($($t:ty),+$(,)?) => {
        $(impl_de_float!($t);)+
    }
}

impl_de_num_signed!(i8, i16, i32, i64, i128, isize);
impl_de_num_unsigned!(u8, u16, u32, u64, u128, usize);
impl_de_float!(f32, f64);

#[macro_export]
#[doc(hidden)]
macro_rules! __const_concat {
    ($a:expr, $b:expr, b">") => {{
        const A: &[u8] = $a;
        const B: &[u8] = $b;
        const __LEN: usize = A.len() + B.len() + 1;
        const __CONCATENATED: &[u8; __LEN] = &{
            let mut out: [u8; __LEN] = ['>' as u8; __LEN];
            let mut i = 0;
            while i < A.len() {
                out[i] = A[i];
                i += 1;
            }
            i = 0;
            while i < B.len() {
                out[i + A.len()] = B[i];
                i += 1;
            }
            out
        };

        __CONCATENATED
    }};
}

/// Serialize and deserialize a Vec of elements.
///
/// The outer type must be a newtype with a single field of type [`std::vec::Vec`]. The inner type
/// must implement [`DeserializeElement`] and [`Serialize`](crate::ser::Serialize).
///
/// # Example
/// ```
/// use xmlib_derive::{Deserialize, Serialize};
///
/// #[derive(Deserialize, Serialize)]
/// pub struct Foo {
///     inner: u8,
/// }
///
/// pub struct Bar(pub Vec<Foo>);
///
/// xmlib::ser_deser_vec!(Bar, b"bar", b"foo");
///
/// let bar = Bar(Vec::from([
///     Foo { inner: 13, },
///     Foo { inner: 42, },
/// ]));
///
/// let serialized = xmlib::ser::write_to_string(&bar).unwrap();
/// assert_eq!(serialized, r#"<bar><foo inner="13"/><foo inner="42"/></bar>"#);
///
/// let deserialized: Bar = xmlib::de::from_str(&serialized).unwrap();
///
/// assert_eq!(deserialized.0.len(), 2);
/// assert_eq!(deserialized.0[0].inner, 13);
/// assert_eq!(deserialized.0[1].inner, 42);
/// ```
#[macro_export]
macro_rules! ser_deser_vec {
    ($name:ident, $tag_name:expr, $inner_tag_name:expr) => {
        impl<W: std::io::Write> $crate::ser::Serialize<W> for $name {
            #[inline]
            fn ser(&self, writer: &mut $crate::ser::XmlWriter<W>) -> std::io::Result<()> {
                const START: &[u8] = $crate::__const_concat!(b"<", $tag_name, b">");
                writer.write_all(START)?;

                for inner in &self.0 {
                    inner.ser(writer)?;
                }

                const END: &[u8] = $crate::__const_concat!(b"</", $tag_name, b">");
                writer.write_all(END)?;
                Ok(())
            }
        }

        impl<R: std::io::BufRead> $crate::de::DeserializeElement<R> for $name {
            #[inline]
            fn name() -> &'static [u8] {
                $tag_name
            }

            #[inline]
            fn de(
                reader: &mut $crate::de::XmlReader<R>,
                _start: quick_xml::events::BytesStart,
            ) -> Result<Self, $crate::de::Error> {
                use quick_xml::events::Event;

                let mut buf = Vec::with_capacity(64);
                let mut inner = Vec::new();

                loop {
                    let event = match reader.read_event(&mut buf) {
                        Ok(event) => event,
                        Err(e) => {
                            return Err($crate::Error {
                                ty_name: String::from_utf8_lossy($tag_name).to_string(),
                                kind: $crate::ErrorKind::XmlError(e),
                            })
                        }
                    };
                    match event {
                        Event::Start(e) if e.local_name() == $inner_tag_name => {
                            inner.push($crate::de::DeserializeElement::de(reader, e)?);
                        }
                        Event::End(e) if e.local_name() == $tag_name => {
                            break;
                        }
                        Event::Text(e) if e.is_empty() => {}
                        e => {
                            return Err($crate::Error {
                                ty_name: String::from_utf8_lossy($tag_name).to_string(),
                                kind: $crate::ErrorKind::UnexpectedEvent(format!("{:?}", e)),
                            })
                        }
                    }
                }

                Ok(Self(inner))
            }
        }
    };
}

/// Deserializes a single struct from a given reader.
///
/// See [`from_str`] for an example.
pub fn deserialize_single_struct<R: std::io::BufRead, T: DeserializeElement<R>>(
    mut reader: XmlReader<R>,
) -> Result<T, Error> {
    use quick_xml::events::Event;
    let mut buf = Vec::with_capacity(32);
    let mut s = None;

    let mut round = 0;
    loop {
        let event = reader.read_event(&mut buf).map_err(|e| Error {
            ty_name: T::name_string(),
            kind: e.into(),
        })?;
        match event {
            Event::Decl(_) => {}
            Event::Start(e) if e.local_name() == T::name() => {
                s = Some(T::de(&mut reader, e)?);
            }
            Event::Eof if s.is_some() => {
                break;
            }
            Event::Text(e) if e.is_empty() => {}
            e => {
                round += 1;
                if round > 10 {
                    panic!(
                        "expected {} got {:?}",
                        String::from_utf8_lossy(T::name()),
                        e
                    );
                }
            }
        }
    }
    s.ok_or_else(|| Error {
        ty_name: T::name_string(),
        kind: ErrorKind::XmlError(quick_xml::Error::UnexpectedEof(String::from(
            "no element found",
        ))),
    })
}

/// Deserializes a single struct from a &str.
///
/// ```
/// use xmlib_derive::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Rectangle {
///     width: u32,
///     height: u32,
/// }
///
/// let serialized = r#"<rectangle width="13" height="42"/>"#;
/// let deserialized: Rectangle = xmlib::de::from_str(&serialized).unwrap();
///
/// assert_eq!(deserialized.width, 13);
/// assert_eq!(deserialized.height, 42);
/// ```
pub fn from_str<'a, T: DeserializeElement<std::io::BufReader<&'a [u8]>>>(
    input: &'a str,
) -> Result<T, Error> {
    let reader = XmlReader::new(std::io::BufReader::new(input.as_bytes()));
    deserialize_single_struct(reader)
}

/// Type which is used to deserialize the namespaces of an element.
pub type CollectNamespaces = Vec<(Vec<u8>, Vec<u8>)>;
