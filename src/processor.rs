use anyhow::Result;
use log::{error, info, warn};
use std::path::Path;

use crate::{
    bankrecords::BankingRecords,
    errors::TransactionError,
    transaction::{Transaction, TransactionType},
};

pub struct TransactionProcessor {}

impl TransactionProcessor {
    pub fn run_transactions_from_csv(csv_path: &Path, records: &mut BankingRecords) -> Result<()> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(csv_path)?;

        for csv_line in csv_reader.deserialize() {
            let transaction: Transaction = csv_line?;
            info!("{transaction:?}");

            let prev_transaction_amount = records.get_transaction(transaction.tx).map(|t| t.amount);

            match transaction.type_ {
                TransactionType::Deposit | TransactionType::Withdrawal => {
                    if prev_transaction_amount.is_some() {
                        warn!(
                            "{}",
                            TransactionError::TransactionAlreadyCompleted(transaction.tx)
                        );
                        continue;
                    }
                }
                _ => {
                    if prev_transaction_amount.is_none() {
                        warn!(
                            "{}",
                            TransactionError::ReferencedTransactionMissing(transaction.tx)
                        );
                        continue;
                    }
                }
            }

            match transaction.type_ {
                TransactionType::Deposit | TransactionType::Withdrawal => (),
                TransactionType::Dispute => {
                    if records.is_disputed(transaction.tx) || records.is_resolved(transaction.tx) {
                        warn!("{}", TransactionError::AlreadyDisputed(transaction.tx));
                        continue;
                    }
                }
                TransactionType::Chargeback => {
                    if !records.is_disputed(transaction.tx) || records.is_resolved(transaction.tx) {
                        warn!("{}", TransactionError::NotYetDisputed(transaction.tx));
                        continue;
                    }
                }
                TransactionType::Resolve => {
                    if records.is_resolved(transaction.tx) {
                        warn!("{}", TransactionError::AlreadyResolved(transaction.tx));
                        continue;
                    }
                    if !records.is_disputed(transaction.tx) {
                        warn!("{}", TransactionError::NotYetDisputed(transaction.tx));
                        continue;
                    }
                }
            }

            let client = records.get_client(transaction.client);
            if client.is_locked() {
                warn!(
                    "{}",
                    TransactionError::TransactionSkippedAccountLocked(
                        transaction.tx,
                        transaction.client
                    )
                );
                continue;
            }

            let transaction_amount = prev_transaction_amount.unwrap_or(transaction.amount);

            if let Err(err) = client.transact(&transaction.type_, transaction_amount) {
                error!("{}", err);
                continue;
            };

            match transaction.type_ {
                TransactionType::Dispute => records.dispute(transaction.tx),
                TransactionType::Resolve | TransactionType::Chargeback => {
                    records.resolve(transaction.tx)
                }
                TransactionType::Deposit | TransactionType::Withdrawal => {
                    records.set_transaction(transaction)
                }
            };
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bankrecords::BankingRecords;
    use crate::client::test_helpers::ClientSnapshot;
    use rstest::{fixture, rstest};
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[fixture]
    fn temp_csv() -> NamedTempFile {
        NamedTempFile::new().unwrap()
    }

    fn write_csv(file: &mut NamedTempFile, content: &str) -> std::path::PathBuf {
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file.path().to_path_buf()
    }

    /// Helper to reduce test boilerplate - processes CSV and returns mutable records
    fn process_csv(mut temp_csv: NamedTempFile, csv_content: &str) -> BankingRecords {
        let path = write_csv(&mut temp_csv, csv_content);
        let mut records = BankingRecords::new(None);
        TransactionProcessor::run_transactions_from_csv(&path, &mut records).unwrap();
        records
    }

    // --- BASIC TRANSACTION TESTS ---
    #[rstest]
    fn test_process_single_deposit(temp_csv: NamedTempFile) {
        let mut records = process_csv(temp_csv, "type,client,tx,amount\ndeposit,1,1,100.0\n");
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 100.0);
        assert_eq!(snap.total, 100.0);
    }

    #[rstest]
    fn test_process_single_withdrawal(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\nwithdrawal,1,2,30.0\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 70.0);
        assert_eq!(snap.total, 70.0);
    }

    #[rstest]
    fn test_withdrawal_insufficient_funds(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\nwithdrawal,1,2,150.0\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 100.0);
        assert_eq!(snap.total, 100.0);
    }

    // --- DISPUTE AND RESOLVE TESTS ---
    #[rstest]
    fn test_dispute_and_resolve(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nresolve,1,1,\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 100.0);
        assert_eq!(snap.held, 0.0);
        assert_eq!(snap.total, 100.0);
    }

    #[rstest]
    fn test_dispute_holds_funds(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 0.0);
        assert_eq!(snap.held, 100.0);
        assert_eq!(snap.total, 100.0);
    }

    // --- CHARGEBACK TESTS ---
    #[rstest]
    fn test_chargeback_locks_account(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nchargeback,1,1,\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert!(snap.locked);
        assert_eq!(snap.available, 0.0);
        assert_eq!(snap.held, 0.0);
        assert_eq!(snap.total, 0.0);
    }

    #[rstest]
    fn test_locked_account_rejects_transactions(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nchargeback,1,1,\ndeposit,1,2,50.0\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert!(snap.locked);
        assert_eq!(snap.available, 0.0);
    }

    // --- MULTIPLE CLIENT TESTS ---
    #[rstest]
    #[case(1, 100.0)]
    #[case(2, 200.0)]
    #[case(3, 300.0)]
    fn test_multiple_clients(temp_csv: NamedTempFile, #[case] client_id: u16, #[case] amount: f64) {
        let csv = format!(
            "type,client,tx,amount\ndeposit,{},{},{}\n",
            client_id, client_id as u32, amount
        );
        let mut records = process_csv(temp_csv, &csv);
        let snap = ClientSnapshot::from_client(records.get_client(client_id));
        assert_eq!(snap.available, amount);
    }

    #[rstest]
    fn test_transactions_isolated_per_client(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\ndeposit,2,2,200.0\ndispute,1,1,\n",
        );
        let snap1 = ClientSnapshot::from_client(records.get_client(1));
        let snap2 = ClientSnapshot::from_client(records.get_client(2));
        assert_eq!(snap1.available, 0.0);
        assert_eq!(snap1.held, 100.0);
        assert_eq!(snap2.available, 200.0);
        assert_eq!(snap2.held, 0.0);
    }

    // --- ERROR HANDLING TESTS ---
    #[rstest]
    fn test_dispute_nonexistent_transaction(temp_csv: NamedTempFile) {
        let mut records = process_csv(temp_csv, "type,client,tx,amount\ndispute,1,999,\n");
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 0.0);
        assert_eq!(snap.held, 0.0);
    }

    #[rstest]
    fn test_resolve_non_disputed_transaction(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\nresolve,1,1,\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 100.0);
    }

    #[rstest]
    fn test_duplicate_deposit_rejected(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\ndeposit,1,1,50.0\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 100.0);
    }

    // --- PRECISION AND FORMATTING TESTS ---
    #[rstest]
    #[case("type,client,tx,amount\ndeposit,1,1,1.5000\n", 1.5)]
    #[case("type,client,tx,amount\ndeposit,1,1,1.5\n", 1.5)]
    #[case("type,client,tx,amount\ndeposit,1,1,1\n", 1.0)]
    #[case("type,client,tx,amount\ndeposit,1,1,100.1234\n", 100.1234)]
    fn test_decimal_precision_variations(
        temp_csv: NamedTempFile,
        #[case] csv_content: &str,
        #[case] expected: f64,
    ) {
        let mut records = process_csv(temp_csv, csv_content);
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert!((snap.available - expected).abs() < 0.0001);
    }

    #[rstest]
    fn test_whitespace_handling(temp_csv: NamedTempFile) {
        let mut records = process_csv(temp_csv, "type, client, tx, amount\ndeposit, 1, 1, 100.0\n");
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 100.0);
    }

    // --- COMPLEX TRANSACTION SEQUENCES ---
    #[rstest]
    fn test_complete_transaction_flow(temp_csv: NamedTempFile) {
        let csv_content = "type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
deposit,1,3,2.0
withdrawal,1,4,1.5
withdrawal,2,5,3.0
dispute,1,1,
resolve,1,1,
dispute,2,2,
chargeback,2,2,
";
        let mut records = process_csv(temp_csv, csv_content);
        let snap1 = ClientSnapshot::from_client(records.get_client(1));
        let snap2 = ClientSnapshot::from_client(records.get_client(2));
        assert_eq!(snap1.available, 1.5);
        assert_eq!(snap1.held, 0.0);
        assert_eq!(snap1.total, 1.5);
        assert!(!snap1.locked);
        assert_eq!(snap2.available, 0.0);
        assert_eq!(snap2.held, 0.0);
        assert_eq!(snap2.total, 0.0);
        assert!(snap2.locked);
    }

    #[rstest]
    fn test_negative_available_funds_allowed(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,50.0\nwithdrawal,1,2,50.0\ndispute,1,1,\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, -50.0);
        assert_eq!(snap.held, 50.0);
        assert_eq!(snap.total, 0.0);
    }

    #[rstest]
    fn test_dispute_then_withdraw_on_held_funds(temp_csv: NamedTempFile) {
        let mut records = process_csv(
            temp_csv,
            "type,client,tx,amount\ndeposit,1,1,100.0\ndispute,1,1,\nwithdrawal,1,2,50.0\n",
        );
        let snap = ClientSnapshot::from_client(records.get_client(1));
        assert_eq!(snap.available, 0.0);
        assert_eq!(snap.held, 100.0);
    }
}
