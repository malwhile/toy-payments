use log::warn;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    bankrecords::serialize_amount, errors::TransactionError, transaction::TransactionType,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Client {
    client: u16,

    #[serde(serialize_with = "serialize_amount")]
    available: Decimal,

    #[serde(serialize_with = "serialize_amount")]
    held: Decimal,

    #[serde(serialize_with = "serialize_amount")]
    total: Decimal,
    locked: bool,
}

impl Client {
    pub fn new(client_id: u16) -> Self {
        Self {
            client: client_id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            total: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn transact(
        &mut self,
        transaction_type: &TransactionType,
        amount: Decimal,
    ) -> Result<(), TransactionError> {
        match transaction_type {
            TransactionType::Deposit => self.deposit(amount),
            TransactionType::Withdrawal => self.withdrawal(amount),
            TransactionType::Dispute => self.dispute(amount),
            TransactionType::Chargeback => self.chargeback(amount),
            TransactionType::Resolve => self.resolve(amount),
        }
    }

    fn deposit(&mut self, amount: Decimal) -> Result<(), TransactionError> {
        self.available = self.available + amount;
        self.total = self.total + amount;
        Ok(())
    }

    fn withdrawal(&mut self, amount: Decimal) -> Result<(), TransactionError> {
        if self.available < amount {
            return Err(TransactionError::NotEnoughFunds);
        }

        self.available = self.available - amount;
        self.total = self.total - amount;

        Ok(())
    }

    fn dispute(&mut self, amount: Decimal) -> Result<(), TransactionError> {
        self.available = self.available - amount;
        self.held = self.held + amount;

        // Flag edge case where available becomes negative—unusual but allowed per spec
        if self.available < Decimal::ZERO {
            warn!("{}", TransactionError::AvailableFundsNegative);
        }

        Ok(())
    }

    fn resolve(&mut self, amount: Decimal) -> Result<(), TransactionError> {
        self.available = self.available + amount;
        self.held = self.held - amount;
        Ok(())
    }

    fn chargeback(&mut self, amount: Decimal) -> Result<(), TransactionError> {
        if self.total < amount {
            return Err(TransactionError::NotEnoughFunds);
        }

        self.held = self.held - amount;
        self.total = self.total - amount;
        self.locked = true;
        Ok(())
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// Test utility to access private Client fields for assertions
    pub struct ClientSnapshot {
        pub available: Decimal,
        pub held: Decimal,
        pub total: Decimal,
        pub locked: bool,
    }

    impl ClientSnapshot {
        pub fn from_client(client: &Client) -> Self {
            ClientSnapshot {
                available: client.available,
                held: client.held,
                total: client.total,
                locked: client.locked,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    // Fixture to create a fresh client for each test
    #[fixture]
    fn client() -> Client {
        Client::new(1)
    }

    // --- DEPOSIT TESTS ---
    #[rstest]
    #[case(Decimal::new(10_000, 4))]
    #[case(Decimal::new(100_000, 4))]
    #[case(Decimal::new(1_005_00, 4))]
    #[case(Decimal::new(100, 2))]
    fn test_deposit_success(mut client: Client, #[case] amount: Decimal) {
        let result = client.deposit(amount);

        assert!(result.is_ok());
        assert_eq!(client.available, amount);
        assert_eq!(client.total, amount);
        assert_eq!(client.held, Decimal::ZERO);
    }

    #[rstest]
    fn test_deposit_multiple_times(mut client: Client) {
        client.deposit(Decimal::new(100_000, 4)).unwrap();
        client.deposit(Decimal::new(50_000, 4)).unwrap();
        client.deposit(Decimal::new(25_000, 4)).unwrap();

        assert_eq!(client.available, Decimal::new(175_000, 4));
        assert_eq!(client.total, Decimal::new(175_000, 4));
    }

    // --- WITHDRAWAL TESTS ---
    #[rstest]
    #[case(Decimal::new(100_000, 4), Decimal::new(50_000, 4))]
    #[case(Decimal::new(1_000_000, 4), Decimal::new(500_000, 4))]
    #[case(Decimal::new(100_000, 4), Decimal::new(100_000, 4))]
    #[case(Decimal::new(1_005_000, 4), Decimal::new(5_000, 4))]
    fn test_withdrawal_success(
        mut client: Client,
        #[case] deposit_amount: Decimal,
        #[case] withdraw_amount: Decimal,
    ) {
        client.deposit(deposit_amount).unwrap();
        let result = client.withdrawal(withdraw_amount);

        assert!(result.is_ok());
        assert_eq!(client.available, deposit_amount - withdraw_amount);
        assert_eq!(client.total, deposit_amount - withdraw_amount);
    }

    #[rstest]
    #[case(Decimal::new(100_000, 4), Decimal::new(150_000, 4))]
    #[case(Decimal::new(50_000, 4), Decimal::new(100_000, 4))]
    #[case(Decimal::new(10_000, 4), Decimal::new(10_100, 4))]
    fn test_withdrawal_insufficient_funds_copy(
        mut client: Client,
        #[case] deposit_amount: Decimal,
        #[case] withdraw_amount: Decimal,
    ) {
        client.deposit(deposit_amount).unwrap();
        let result = client.withdrawal(withdraw_amount);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TransactionError::NotEnoughFunds);
        assert_eq!(client.available, deposit_amount);
        assert_eq!(client.total, deposit_amount);
    }

    #[test]
    fn test_withdrawal_from_empty_account() {
        let mut client = Client::new(1);
        let result = client.withdrawal(Decimal::new(50_000, 4));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TransactionError::NotEnoughFunds);
    }

    // --- DISPUTE TESTS ---
    #[rstest]
    #[case(Decimal::new(2_000_000, 4), Decimal::new(500_000, 4))]
    #[case(Decimal::new(1_000_000, 4), Decimal::new(500_000, 4))]
    #[case(Decimal::new(100_000, 4), Decimal::new(100_000, 4))]
    fn test_dispute_success(
        mut client: Client,
        #[case] deposit_amount: Decimal,
        #[case] dispute_amount: Decimal,
    ) {
        client.deposit(deposit_amount).unwrap();
        let result = client.dispute(dispute_amount);

        assert!(result.is_ok());
        assert_eq!(client.available, deposit_amount - dispute_amount);
        assert_eq!(client.held, dispute_amount);
        assert_eq!(client.total, deposit_amount);
    }

    #[rstest]
    #[case(Decimal::new(100_000, 4), Decimal::new(1_500_000, 4))]
    #[case(Decimal::new(500_000, 4), Decimal::new(1_000_000, 4))]
    fn test_dispute_available_goes_negative(
        mut client: Client,
        #[case] deposit_amount: Decimal,
        #[case] dispute_amount: Decimal,
    ) {
        client.deposit(deposit_amount).unwrap();
        let result = client.dispute(dispute_amount);

        // Dispute succeeds even if available goes negative (allowed per spec)
        assert!(result.is_ok());
        assert_eq!(client.available, deposit_amount - dispute_amount);
        assert_eq!(client.held, dispute_amount);
    }

    #[test]
    fn test_dispute_on_empty_account() {
        let mut client = Client::new(1);
        let result = client.dispute(Decimal::new(50_000, 4));

        // Dispute succeeds even on empty account (allowed per spec, just warns)
        assert!(result.is_ok());
        assert_eq!(client.available, Decimal::new(-50_000, 4));
        assert_eq!(client.held, Decimal::new(50_000, 4));
    }

    // --- RESOLVE TESTS ---
    #[rstest]
    #[case(Decimal::new(2_000_000, 4), Decimal::new(500_000, 4))]
    #[case(Decimal::new(1_000_000, 4), Decimal::new(500_000, 4))]
    #[case(Decimal::new(100_000, 4), Decimal::new(100_000, 4))]
    fn test_resolve_success(
        mut client: Client,
        #[case] deposit_amount: Decimal,
        #[case] dispute_amount: Decimal,
    ) {
        client.deposit(deposit_amount).unwrap();
        client.dispute(dispute_amount).unwrap();

        let result = client.resolve(dispute_amount);

        assert!(result.is_ok());
        assert_eq!(client.available, deposit_amount);
        assert_eq!(client.held, Decimal::ZERO);
        assert_eq!(client.total, deposit_amount);
    }

    #[test]
    fn test_resolve_with_multiple_disputes() {
        let mut client = Client::new(1);
        client.deposit(Decimal::new(1_000_000, 4)).unwrap();
        client.dispute(Decimal::new(200_000, 4)).unwrap();
        client.dispute(Decimal::new(300_000, 4)).unwrap();

        client.resolve(Decimal::new(200_000, 4)).unwrap();

        assert_eq!(client.available, Decimal::new(700_000, 4)); // 100 - 20 - 30 + 20 = 70
        assert_eq!(client.held, Decimal::new(300_000, 4)); // 20 + 30 - 20 = 30
        assert_eq!(client.total, Decimal::new(1_000_000, 4));
    }

    // --- CHARGEBACK TESTS ---
    #[rstest]
    #[case(Decimal::new(2_000_000, 4), Decimal::new(500_000, 4))]
    #[case(Decimal::new(1_000_000, 4), Decimal::new(500_000, 4))]
    #[case(Decimal::new(100_000, 4), Decimal::new(100_000, 4))]
    fn test_chargeback_success(
        mut client: Client,
        #[case] deposit_amount: Decimal,
        #[case] chargeback_amount: Decimal,
    ) {
        client.deposit(deposit_amount).unwrap();
        client.dispute(chargeback_amount).unwrap();

        let result = client.chargeback(chargeback_amount);

        assert!(result.is_ok());
        assert_eq!(client.total, deposit_amount - chargeback_amount);
        assert_eq!(client.held, Decimal::ZERO);
        assert!(client.is_locked());
    }

    #[rstest]
    #[case(Decimal::new(100_000, 4), Decimal::new(1_500_000, 4))]
    #[case(Decimal::new(50_000, 4), Decimal::new(1_000_000, 4))]
    fn test_chargeback_insufficient_total(
        mut client: Client,
        #[case] deposit_amount: Decimal,
        #[case] chargeback_amount: Decimal,
    ) {
        client.deposit(deposit_amount).unwrap();

        let result = client.chargeback(chargeback_amount);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TransactionError::NotEnoughFunds);
        assert_eq!(client.total, deposit_amount);
        assert!(!client.is_locked());
    }

    #[test]
    fn test_chargeback_on_empty_account() {
        let mut client = Client::new(1);
        let result = client.chargeback(Decimal::new(50_000, 4));

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TransactionError::NotEnoughFunds);
        assert!(!client.is_locked());
    }

    // --- CLIENT INITIALIZATION ---
    #[test]
    fn test_new_client_initialization() {
        let client = Client::new(42);

        assert_eq!(client.client, 42);
        assert_eq!(client.available, Decimal::ZERO);
        assert_eq!(client.held, Decimal::ZERO);
        assert_eq!(client.total, Decimal::ZERO);
        assert!(!client.is_locked());
    }

    // --- LOCK STATE TESTS ---
    #[test]
    fn test_account_locks_after_chargeback() {
        let mut client = Client::new(1);
        client.deposit(Decimal::new(5_000_000, 4)).unwrap();
        client.dispute(Decimal::new(3_000_000, 4)).unwrap();
        client.chargeback(Decimal::new(3_000_000, 4)).unwrap();

        assert!(client.is_locked());
    }

    // --- INTEGRATION TESTS ---
    #[test]
    fn test_full_transaction_flow() {
        let mut client = Client::new(1);

        client.deposit(Decimal::new(1_000_000, 4)).unwrap();
        assert_eq!(client.total, Decimal::new(1_000_000, 4));
        assert_eq!(client.available, Decimal::new(1_000_000, 4));

        client.withdrawal(Decimal::new(300_000, 4)).unwrap();
        assert_eq!(client.total, Decimal::new(700_000, 4));
        assert_eq!(client.available, Decimal::new(700_000, 4));

        client.dispute(Decimal::new(200_000, 4)).unwrap();
        assert_eq!(client.total, Decimal::new(700_000, 4));
        assert_eq!(client.available, Decimal::new(500_000, 4));
        assert_eq!(client.held, Decimal::new(200_000, 4));

        client.resolve(Decimal::new(200_000, 4)).unwrap();
        assert_eq!(client.total, Decimal::new(700_000, 4));
        assert_eq!(client.available, Decimal::new(700_000, 4));
        assert_eq!(client.held, Decimal::ZERO);
    }
}
