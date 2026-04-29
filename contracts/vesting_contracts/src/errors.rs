use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    // // General (100s)
    Unauthorized = 100,
    InvalidInput = 101,
    AlreadyInitialized = 102,
    NotInitialized = 103,
    ContractPaused = 104,
    ContractDeprecated = 105,

    // Vesting (200s)
    VestingNotFound = 200,
    VaultNotFound = 201,
    CliffNotReached = 202,
    NothingToClaim = 203,
    AlreadyFullyClaimed = 204,
    VaultRevoked = 205,
    VaultFrozen = 206,
    InvalidSchedule = 207,
    StepDurationInvalid = 208,
    InvalidAmount = 209,
    VaultNotInitialized = 210,
    MilestoneNotCompleted = 211,
    MilestoneAlreadyTriggered = 212,
    MilestoneNotSequential = 213,
    VestingExpired = 214,
    CliffJumpTooLarge = 215,

    // Financial (300s)
    InsufficientBalance = 300,
    InsufficientFunds = 301,
    TransferFailed = 302,
    AllowanceExceeded = 303,

    // Compliance (400s)
    KycNotCompleted = 400,
    KycExpired = 401,
    AddressSanctioned = 402,
    JurisdictionRestricted = 403,
    LegalSignatureMissing = 404,
    LegalSignatureInvalid = 405,
    ComplianceCheckFailed = 406,
    AmlThresholdExceeded = 407,
    RiskRatingTooHigh = 408,
    DocumentVerificationFailed = 409,
    AccreditationStatusInvalid = 410,
    TaxComplianceFailed = 411,
    RegulatoryBlockActive = 412,
    WhitelistNotApproved = 413,
    BlacklistViolation = 414,
    GeofencingRestriction = 415,
    IdentityVerificationExpired = 416,
    SourceOfFundsNotVerified = 417,
    BeneficialOwnerNotVerified = 418,
    PoliticallyExposedPerson = 419,
    SanctionsListHit = 420,

    // Governance (500s)
    ProposalNotFound = 500,
    AlreadyVoted = 501,
    VotingPeriodEnded = 502,
    ProposalExpired = 503,
    QuorumNotMet = 504,
    InvalidVote = 505,

    // Staking (600s)
    StakeNotFound = 600,
    AlreadyStaked = 601,
    UnstakingNotPossible = 602,
    StakeAmountInvalid = 603,
    StakingPeriodInvalid = 604,

    // Multi-sig (700s)
    MultisigNotActive = 700,
    ProposalAlreadyExecuted = 701,
    InsufficientSignatures = 702,
    InvalidProposal = 703,
    NotMultisigMember = 704,

    // Certificate Registry (800s)
    CertificateNotFound = 800,
    CertificateAlreadyExists = 801,
    InvalidCertificate = 802,
    CertificateExpired = 803,

    // Path Payment (1000s)
    PathPaymentNotConfigured = 1000,
    PathPaymentDisabled = 1001,
    InsufficientLiquidity = 1002,
    PathPaymentFailed = 1003,

    // System (900s)
    Overflow = 900,
    Underflow = 901,
    DivisionByZero = 902,
    InternalError = 903,
    ReentrancyDetected = 904,
}
