use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransactionError {
    #[error("Not enough funds to complete transaction.")]
    NotEnoughFunds,

    #[error("Dispute has made available funds negative, please review before resolving.")]
    AvailableFundsNegative,

    #[error("The referenced transaction {0}, does not exist")]
    ReferencedTransactionMissing(u32),

    #[error("The referenced transaction {0}, already ran")]
    TransactionAlreadyCompleted(u32),

    #[error("The referenced transaction {0}, not completed, account {1} is locked")]
    TransactionSkippedAccountLocked(u32, u16),

    #[error("Transaction {0}, has already been disputed")]
    AlreadyDisputed(u32),

    #[error("Transaction {0}, not yet disputed")]
    NotYetDisputed(u32),

    #[error("Transaction {0}, has already been resolved")]
    AlreadyResolved(u32),
}
