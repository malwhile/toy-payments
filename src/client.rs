use log::warn;
use serde::{Deserialize, Serialize};

use crate::{
    bankrecords::{round_amount, serialize_amount},
    errors::TransactionError,
    transaction::TransactionType,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Client {
    client: u16,

    #[serde(serialize_with = "serialize_amount")]
    available: f64,

    #[serde(serialize_with = "serialize_amount")]
    held: f64,

    #[serde(serialize_with = "serialize_amount")]
    total: f64,
    locked: bool,
}

impl Client {
    pub fn new(client_id: u16) -> Self {
        Self {
            client: client_id,
            available: 0.0,
            held: 0.0,
            total: 0.0,
            locked: false,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn transact(
        &mut self,
        transaction_type: &TransactionType,
        amount: f64,
    ) -> Result<(), TransactionError> {
        match transaction_type {
            TransactionType::Deposit => self.deposit(amount),
            TransactionType::Withdrawal => self.withdrawal(amount),
            TransactionType::Dispute => self.dispute(amount),
            TransactionType::Chargeback => self.chargeback(amount),
            TransactionType::Resolve => self.resolve(amount),
        }
    }

    fn deposit(&mut self, amount: f64) -> Result<(), TransactionError> {
        self.available = round_amount(self.available + amount);
        self.total = round_amount(self.total + amount);
        Ok(())
    }

    fn withdrawal(&mut self, amount: f64) -> Result<(), TransactionError> {
        if self.available < amount {
            return Err(TransactionError::NotEnoughFunds);
        }

        self.available = round_amount(self.available - amount);
        self.total = round_amount(self.total - amount);

        Ok(())
    }

    fn dispute(&mut self, amount: f64) -> Result<(), TransactionError> {
        self.available = round_amount(self.available - amount);
        self.held = round_amount(self.held + amount);

        // Flag edge case where available becomes negative—unusual but allowed per spec
        if self.available < 0.0 {
            warn!("{}", TransactionError::AvailableFundsNegative);
        }

        Ok(())
    }

    fn resolve(&mut self, amount: f64) -> Result<(), TransactionError> {
        self.available = round_amount(self.available + amount);
        self.held = round_amount(self.held - amount);
        Ok(())
    }

    fn chargeback(&mut self, amount: f64) -> Result<(), TransactionError> {
        if self.total < amount {
            return Err(TransactionError::NotEnoughFunds);
        }

        self.held = round_amount(self.held - amount);
        self.total = round_amount(self.total - amount);
        self.locked = true;
        Ok(())
    }
}

#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// Test utility to access private Client fields for assertions
    pub struct ClientSnapshot {
        pub available: f64,
        pub held: f64,
        pub total: f64,
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
    #[case(1.0)]
    #[case(10.0)]
    #[case(100.5)]
    #[case(0.01)]
    fn test_deposit_success(mut client: Client, #[case] amount: f64) {
        let result = client.deposit(amount);

        assert!(result.is_ok());
        assert_eq!(client.available, amount);
        assert_eq!(client.total, amount);
        assert_eq!(client.held, 0.0);
    }

    #[rstest]
    fn test_deposit_multiple_times(mut client: Client) {
        client.deposit(10.0).unwrap();
        client.deposit(5.0).unwrap();
        client.deposit(2.5).unwrap();

        assert_eq!(client.available, 17.5);
        assert_eq!(client.total, 17.5);
    }

    // --- WITHDRAWAL TESTS ---
    #[rstest]
    #[case(10.0, 5.0)]
    #[case(100.0, 50.0)]
    #[case(10.0, 10.0)]
    #[case(100.5, 0.5)]
    fn test_withdrawal_success(
        mut client: Client,
        #[case] deposit_amount: f64,
        #[case] withdraw_amount: f64,
    ) {
        client.deposit(deposit_amount).unwrap();
        let result = client.withdrawal(withdraw_amount);

        assert!(result.is_ok());
        assert_eq!(client.available, deposit_amount - withdraw_amount);
        assert_eq!(client.total, deposit_amount - withdraw_amount);
    }

    #[rstest]
    #[case(10.0, 15.0)]
    #[case(5.0, 10.0)]
    #[case(1.0, 1.01)]
    fn test_withdrawal_insufficient_funds(
        mut client: Client,
        #[case] deposit_amount: f64,
        #[case] withdraw_amount: f64,
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
        let result = client.withdrawal(5.0);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TransactionError::NotEnoughFunds);
    }

    // --- DISPUTE TESTS ---
    #[rstest]
    #[case(20.0, 5.0)]
    #[case(100.0, 50.0)]
    #[case(10.0, 10.0)]
    fn test_dispute_success(
        mut client: Client,
        #[case] deposit_amount: f64,
        #[case] dispute_amount: f64,
    ) {
        client.deposit(deposit_amount).unwrap();
        let result = client.dispute(dispute_amount);

        assert!(result.is_ok());
        assert_eq!(client.available, deposit_amount - dispute_amount);
        assert_eq!(client.held, dispute_amount);
        assert_eq!(client.total, deposit_amount);
    }

    #[rstest]
    #[case(10.0, 15.0)]
    #[case(5.0, 10.0)]
    fn test_dispute_available_goes_negative(
        mut client: Client,
        #[case] deposit_amount: f64,
        #[case] dispute_amount: f64,
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
        let result = client.dispute(5.0);

        // Dispute succeeds even on empty account (allowed per spec, just warns)
        assert!(result.is_ok());
        assert_eq!(client.available, -5.0);
        assert_eq!(client.held, 5.0);
    }

    // --- RESOLVE TESTS ---
    #[rstest]
    #[case(20.0, 5.0)]
    #[case(100.0, 50.0)]
    #[case(10.0, 10.0)]
    fn test_resolve_success(
        mut client: Client,
        #[case] deposit_amount: f64,
        #[case] dispute_amount: f64,
    ) {
        client.deposit(deposit_amount).unwrap();
        client.dispute(dispute_amount).unwrap();

        let result = client.resolve(dispute_amount);

        assert!(result.is_ok());
        assert_eq!(client.available, deposit_amount);
        assert_eq!(client.held, 0.0);
        assert_eq!(client.total, deposit_amount);
    }

    #[test]
    fn test_resolve_with_multiple_disputes() {
        let mut client = Client::new(1);
        client.deposit(100.0).unwrap();
        client.dispute(20.0).unwrap();
        client.dispute(30.0).unwrap();

        client.resolve(20.0).unwrap();

        assert_eq!(client.available, 70.0); // 100 - 20 - 30 + 20 = 70
        assert_eq!(client.held, 30.0); // 20 + 30 - 20 = 30
        assert_eq!(client.total, 100.0);
    }

    // --- CHARGEBACK TESTS ---
    #[rstest]
    #[case(20.0, 5.0)]
    #[case(100.0, 50.0)]
    #[case(10.0, 10.0)]
    fn test_chargeback_success(
        mut client: Client,
        #[case] deposit_amount: f64,
        #[case] chargeback_amount: f64,
    ) {
        client.deposit(deposit_amount).unwrap();
        client.dispute(chargeback_amount).unwrap();

        let result = client.chargeback(chargeback_amount);

        assert!(result.is_ok());
        assert_eq!(client.total, deposit_amount - chargeback_amount);
        assert_eq!(client.held, 0.0);
        assert!(client.is_locked());
    }

    #[rstest]
    #[case(10.0, 15.0)]
    #[case(5.0, 10.0)]
    fn test_chargeback_insufficient_total(
        mut client: Client,
        #[case] deposit_amount: f64,
        #[case] chargeback_amount: f64,
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
        let result = client.chargeback(5.0);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TransactionError::NotEnoughFunds);
        assert!(!client.is_locked());
    }

    // --- CLIENT INITIALIZATION ---
    #[test]
    fn test_new_client_initialization() {
        let client = Client::new(42);

        assert_eq!(client.client, 42);
        assert_eq!(client.available, 0.0);
        assert_eq!(client.held, 0.0);
        assert_eq!(client.total, 0.0);
        assert!(!client.is_locked());
    }

    // --- LOCK STATE TESTS ---
    #[test]
    fn test_account_locks_after_chargeback() {
        let mut client = Client::new(1);
        client.deposit(50.0).unwrap();
        client.dispute(30.0).unwrap();
        client.chargeback(30.0).unwrap();

        assert!(client.is_locked());
    }

    // --- INTEGRATION TESTS ---
    #[test]
    fn test_full_transaction_flow() {
        let mut client = Client::new(1);

        client.deposit(100.0).unwrap();
        assert_eq!(client.total, 100.0);
        assert_eq!(client.available, 100.0);

        client.withdrawal(30.0).unwrap();
        assert_eq!(client.total, 70.0);
        assert_eq!(client.available, 70.0);

        client.dispute(20.0).unwrap();
        assert_eq!(client.total, 70.0);
        assert_eq!(client.available, 50.0);
        assert_eq!(client.held, 20.0);

        client.resolve(20.0).unwrap();
        assert_eq!(client.total, 70.0);
        assert_eq!(client.available, 70.0);
        assert_eq!(client.held, 0.0);
    }
}
