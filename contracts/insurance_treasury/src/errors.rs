use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    UnauthorizedDeposit = 1,
    UnsupportedAsset = 2,
    UnauthorizedBailoutAccess = 3,
    InsufficientFunds = 4,
    RequestAlreadyExecuted = 5,
    TimelockNotExpired = 6,
    InsufficientSignatures = 7,
}