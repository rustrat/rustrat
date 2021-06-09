pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    FfiError(crate::ffi::error::Error),
    WasmError(wasm3::error::Error),
    IoError(std::io::Error),
    SerializationError(bincode::Error),
    HttpError(crate::connector::http::error::Error),
    // TODO rename? Use module specific errors?
    ArgumentError,
    EncryptionError(chacha20poly1305::aead::Error),
    CheckinFailed(rustrat_common::encryption::PublicKey),
    InvalidStr(std::str::Utf8Error),
    FunctionDoesNotExist(String),
}

impl From<crate::ffi::error::Error> for Error {
    fn from(error: crate::ffi::error::Error) -> Self {
        Error::FfiError(error)
    }
}

impl From<wasm3::error::Error> for Error {
    fn from(error: wasm3::error::Error) -> Self {
        Error::WasmError(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IoError(error)
    }
}

impl From<bincode::Error> for Error {
    fn from(error: bincode::Error) -> Self {
        Error::SerializationError(error)
    }
}

impl From<crate::connector::http::error::Error> for Error {
    fn from(error: crate::connector::http::error::Error) -> Self {
        Error::HttpError(error)
    }
}

impl From<rustrat_common::error::Error> for Error {
    fn from(error: rustrat_common::error::Error) -> Self {
        match error {
            rustrat_common::error::Error::EncryptionError(e) => (Error::EncryptionError(e)),
            rustrat_common::error::Error::SerializationError(e) => Error::SerializationError(e),
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(error: std::str::Utf8Error) -> Self {
        Error::InvalidStr(error)
    }
}
