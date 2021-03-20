pub enum Error {
    PublicKeyAlreadyExistsOnCheckin,
    PublicKeyDoesNotExist,
    DecryptionError,
    UnknownEnumValue,
}

pub type Result<T> = std::result::Result<T, Error>;