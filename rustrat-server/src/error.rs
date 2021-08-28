pub enum Error {
    PublicKeyAlreadyExistsOnCheckin,
    PublicKeyDoesNotExist,
    DecryptionError,
    UnknownEnumValue,
    CommunicationError,
    DbError,
}

pub type Result<T> = std::result::Result<T, Error>;
