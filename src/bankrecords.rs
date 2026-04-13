// In production, this would be stored to a DB
// For this project we will hold everything in memory
// And dumpt to CSVs for persistance

use anyhow::Result;
use csv::Writer;
use serde::{Deserialize, Deserializer, Serializer};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};

use crate::{client::Client, transaction::Transaction};

pub fn round_amount(value: f64) -> f64 {
    (value * 10000.0).round() / 10000.0
}

pub fn serialize_amount<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let rounded = round_amount(*value);
    serializer.serialize_f64(rounded)
}

pub fn deserialize_amount<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        return Ok(0.0);
    }

    let amount: f64 = s.parse::<f64>().map_err(serde::de::Error::custom)?;
    Ok(round_amount(amount))
}

pub struct BankingRecords {
    clients: BTreeMap<u16, Client>,
    transactions: HashMap<u32, Transaction>,
    disputed: HashSet<u32>,
    resolved: HashSet<u32>,
}

impl BankingRecords {
    pub fn new(_records_path: Option<&Path>) -> Self {
        Self {
            clients: BTreeMap::new(),
            transactions: HashMap::new(),
            disputed: HashSet::new(),
            resolved: HashSet::new(),
        }
    }

    pub fn clients_to_csv(&self) -> Result<String> {
        let mut csv_writer = Writer::from_writer(vec![]);

        for client in self.clients.values() {
            csv_writer.serialize(client)?;
        }
        csv_writer.flush()?;

        let client_csv = csv_writer.into_inner()?;

        Ok(String::from_utf8(client_csv)?)
    }

    pub fn get_transaction(&self, tx_id: u32) -> Option<&Transaction> {
        self.transactions.get(&tx_id)
    }

    pub fn dispute(&mut self, tx_id: u32) {
        if self.transactions.contains_key(&tx_id) && !self.resolved.contains(&tx_id) {
            self.disputed.insert(tx_id);
        }
    }

    pub fn is_disputed(&self, tx_id: u32) -> bool {
        self.disputed.contains(&tx_id)
    }

    pub fn resolve(&mut self, tx_id: u32) {
        if self.disputed.remove(&tx_id) {
            self.resolved.insert(tx_id);
        }
    }

    pub fn is_resolved(&self, tx_id: u32) -> bool {
        self.resolved.contains(&tx_id)
    }

    pub fn set_transaction(&mut self, transaction: Transaction) {
        self.transactions.insert(transaction.tx, transaction);
    }

    pub fn get_client(&mut self, client_id: u16) -> &mut Client {
        self.clients
            .entry(client_id)
            .or_insert(Client::new(client_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::TransactionType;
    use rstest::{fixture, rstest};

    #[fixture]
    fn records() -> BankingRecords {
        BankingRecords::new(None)
    }

    #[fixture]
    fn sample_transaction() -> Transaction {
        Transaction {
            type_: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: 100.0,
        }
    }

    // --- TRANSACTION STORAGE TESTS ---
    #[rstest]
    #[case(1, 1, 100.0)]
    #[case(2, 5, 250.5)]
    #[case(65535, 4294967295, 9999.9999)]
    fn test_set_and_get_transaction(
        mut records: BankingRecords,
        #[case] client_id: u16,
        #[case] tx_id: u32,
        #[case] amount: f64,
    ) {
        let transaction = Transaction {
            type_: TransactionType::Deposit,
            client: client_id,
            tx: tx_id,
            amount,
        };

        records.set_transaction(transaction.clone());
        let retrieved = records.get_transaction(tx_id);

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().tx, tx_id);
        assert_eq!(retrieved.unwrap().client, client_id);
    }

    #[rstest]
    fn test_get_nonexistent_transaction(records: BankingRecords) {
        let result = records.get_transaction(999);
        assert!(result.is_none());
    }

    #[rstest]
    fn test_set_transaction_overwrites(
        mut records: BankingRecords,
        sample_transaction: Transaction,
    ) {
        let tx_id = sample_transaction.tx;

        records.set_transaction(sample_transaction);
        let first = records.get_transaction(tx_id).unwrap().amount;

        let updated_transaction = Transaction {
            type_: TransactionType::Withdrawal,
            client: 1,
            tx: tx_id,
            amount: 50.0,
        };
        records.set_transaction(updated_transaction);
        let second = records.get_transaction(tx_id).unwrap().amount;

        assert_eq!(first, 100.0);
        assert_eq!(second, 50.0);
    }

    // --- DISPUTE STATE TESTS ---
    #[rstest]
    fn test_dispute_marks_transaction(
        mut records: BankingRecords,
        sample_transaction: Transaction,
    ) {
        records.set_transaction(sample_transaction);

        assert!(!records.is_disputed(1));
        records.dispute(1);
        assert!(records.is_disputed(1));
    }

    #[rstest]
    fn test_dispute_nonexistent_transaction(mut records: BankingRecords) {
        // Disputing a nonexistent transaction should be safe (no-op)
        records.dispute(999);
        assert!(!records.is_disputed(999));
    }

    #[rstest]
    fn test_dispute_already_resolved(mut records: BankingRecords, sample_transaction: Transaction) {
        records.set_transaction(sample_transaction);

        records.dispute(1);
        records.resolve(1);

        // Trying to dispute an already resolved transaction should not mark it
        records.dispute(1);
        assert!(!records.is_disputed(1));
        assert!(records.is_resolved(1));
    }

    #[rstest]
    #[case(1)]
    #[case(100)]
    #[case(4294967295)] // u32::MAX
    fn test_dispute_multiple_transactions(mut records: BankingRecords, #[case] tx_id: u32) {
        let tx1 = Transaction {
            type_: TransactionType::Deposit,
            client: 1,
            tx: tx_id,
            amount: 100.0,
        };
        let tx2 = Transaction {
            type_: TransactionType::Deposit,
            client: 1,
            tx: tx_id.wrapping_add(1),
            amount: 50.0,
        };

        records.set_transaction(tx1);
        records.set_transaction(tx2);

        records.dispute(tx_id);
        records.dispute(tx_id.wrapping_add(1));

        assert!(records.is_disputed(tx_id));
        assert!(records.is_disputed(tx_id.wrapping_add(1)));
    }

    // --- RESOLVE STATE TESTS ---
    #[rstest]
    fn test_resolve_removes_from_disputed(
        mut records: BankingRecords,
        sample_transaction: Transaction,
    ) {
        records.set_transaction(sample_transaction);
        records.dispute(1);

        assert!(records.is_disputed(1));
        records.resolve(1);
        assert!(!records.is_disputed(1));
        assert!(records.is_resolved(1));
    }

    #[rstest]
    fn test_resolve_nonexistent_transaction(mut records: BankingRecords) {
        // Resolving a nonexistent transaction should be safe
        records.resolve(999);
        assert!(!records.is_resolved(999));
        assert!(!records.is_disputed(999));
    }

    #[rstest]
    fn test_resolve_non_disputed_transaction(
        mut records: BankingRecords,
        sample_transaction: Transaction,
    ) {
        records.set_transaction(sample_transaction);

        // Trying to resolve a transaction that was never disputed
        records.resolve(1);
        assert!(!records.is_resolved(1));
        assert!(!records.is_disputed(1));
    }

    #[rstest]
    fn test_is_resolved_without_dispute(records: BankingRecords) {
        assert!(!records.is_resolved(1));
    }

    // --- CLIENT MANAGEMENT TESTS ---
    #[rstest]
    #[case(1)]
    #[case(100)]
    #[case(65535)]
    fn test_get_client_creates_new(mut records: BankingRecords, #[case] client_id: u16) {
        use crate::client::test_helpers::ClientSnapshot;

        let client = records.get_client(client_id);
        let snap = ClientSnapshot::from_client(client);

        assert_eq!(snap.available, 0.0);
        assert_eq!(snap.held, 0.0);
        assert_eq!(snap.total, 0.0);
        assert!(!snap.locked);
    }

    #[rstest]
    fn test_get_client_returns_same_reference(mut records: BankingRecords) {
        use crate::client::test_helpers::ClientSnapshot;
        use crate::transaction::TransactionType;

        {
            let client = records.get_client(1);
            let _ = client.transact(&TransactionType::Deposit, 100.0);
        }

        let client = records.get_client(1);
        let snap = ClientSnapshot::from_client(client);
        assert_eq!(snap.available, 100.0);
    }

    #[rstest]
    fn test_multiple_clients(mut records: BankingRecords) {
        use crate::client::test_helpers::ClientSnapshot;
        use crate::transaction::TransactionType;

        {
            let client = records.get_client(1);
            let _ = client.transact(&TransactionType::Deposit, 100.0);
        }

        {
            let client = records.get_client(2);
            let _ = client.transact(&TransactionType::Deposit, 50.0);
        }

        {
            let client = records.get_client(3);
            let _ = client.transact(&TransactionType::Deposit, 75.0);
        }

        let snap1 = ClientSnapshot::from_client(records.get_client(1));
        let snap2 = ClientSnapshot::from_client(records.get_client(2));
        let snap3 = ClientSnapshot::from_client(records.get_client(3));

        assert_eq!(snap1.available, 100.0);
        assert_eq!(snap2.available, 50.0);
        assert_eq!(snap3.available, 75.0);
    }

    // --- CSV OUTPUT TESTS ---
    #[rstest]
    fn test_clients_to_csv_empty(mut records: BankingRecords) {
        // Access a client to ensure the records structure is populated
        let _ = records.get_client(1);
        let csv = records.clients_to_csv().unwrap();
        // CSV should have header
        assert!(csv.contains("client"));
        assert!(csv.contains("available"));
        assert!(csv.contains("held"));
        assert!(csv.contains("total"));
        assert!(csv.contains("locked"));
    }

    #[rstest]
    fn test_clients_to_csv_single_client(mut records: BankingRecords) {
        use crate::transaction::TransactionType;

        let client = records.get_client(1);
        let _ = client.transact(&TransactionType::Deposit, 100.0);

        let csv = records.clients_to_csv().unwrap();
        assert!(csv.contains("1,100"));
    }

    #[rstest]
    fn test_clients_to_csv_multiple_clients(mut records: BankingRecords) {
        use crate::transaction::TransactionType;

        {
            let c1 = records.get_client(1);
            let _ = c1.transact(&TransactionType::Deposit, 100.0);
        }

        {
            let c2 = records.get_client(2);
            let _ = c2.transact(&TransactionType::Deposit, 50.0);
            let _ = c2.transact(&TransactionType::Withdrawal, 20.0);
        }

        let csv = records.clients_to_csv().unwrap();
        assert!(csv.contains("1,100"));
        assert!(csv.contains("2,30"));
    }

    // --- INTEGRATION TESTS ---
    #[rstest]
    fn test_full_dispute_lifecycle(mut records: BankingRecords) {
        let tx = Transaction {
            type_: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: 100.0,
        };

        records.set_transaction(tx);
        assert!(!records.is_disputed(1));
        assert!(!records.is_resolved(1));

        records.dispute(1);
        assert!(records.is_disputed(1));
        assert!(!records.is_resolved(1));

        records.resolve(1);
        assert!(!records.is_disputed(1));
        assert!(records.is_resolved(1));
    }

    #[rstest]
    fn test_cannot_dispute_after_resolve(mut records: BankingRecords) {
        let tx = Transaction {
            type_: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: 100.0,
        };

        records.set_transaction(tx);
        records.dispute(1);
        records.resolve(1);

        // This should not mark it as disputed again (due to is_resolved check)
        records.dispute(1);
        assert!(!records.is_disputed(1));
    }
}
