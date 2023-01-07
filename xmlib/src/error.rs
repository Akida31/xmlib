use std::fmt::{self, Debug, Display, Formatter};

/// This type represents all possible errors that can occur.
pub struct Error {
    /// Name of the element in which the error occurred.
    pub ty_name: String,
    /// Errorkind which contains additional data.
    pub kind: ErrorKind,
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "xml error in type {}: {}", self.ty_name, self.kind)
    }
}

impl std::error::Error for Error {}

/// A list specifying kinds of error.
///
/// It is used with the [`Error`] type.
///
/// Note that this contains currently formatted strings, whose exact representation should not be
/// relied upon.
pub enum ErrorKind {
    /// Error from [`quick_xml`]
    XmlError(quick_xml::Error),
    /// Invalid data for type
    InvalidType(String),
    /// Required attribute of struct was missing
    MissingAttr(String),
    /// Invalid event occurred while deserialization
    UnexpectedEvent(String),
    /// Validation of attribute failed
    Validation(String),
    /// Could not convert bytes to valid utf8 string
    FromUtf8Error(std::string::FromUtf8Error),
}

impl From<quick_xml::Error> for ErrorKind {
    fn from(value: quick_xml::Error) -> Self {
        Self::XmlError(value)
    }
}

impl Debug for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::XmlError(e) => write!(f, "xml error: {}", e),
            Self::InvalidType(e) => write!(f, "invalid type: {}", e),
            Self::MissingAttr(e) => write!(f, "missing attribute: {}", e),
            Self::UnexpectedEvent(e) => write!(f, "unexpected event: {}", e),
            Self::Validation(e) => write!(f, "failed validation: {}", e),
            Self::FromUtf8Error(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ErrorKind {}

impl From<quick_xml::events::attributes::AttrError> for ErrorKind {
    fn from(e: quick_xml::events::attributes::AttrError) -> Self {
        Self::XmlError(quick_xml::Error::InvalidAttr(e))
    }
}
