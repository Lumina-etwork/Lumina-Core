use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    ConfigNotFound = 2,
    SchemaValidationFailed = 3,
    ConfigAlreadyExists = 4,
    InvalidVersion = 5,
    NotInitialized = 6,
    AlreadyInitialized = 7,
    ConfigNotModified = 8,
}
