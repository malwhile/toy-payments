use anyhow::Result;
use clap::Parser;
use log::{LevelFilter, info};
use std::path::Path;

use crate::{bankrecords::BankingRecords, processor::TransactionProcessor};

mod bankrecords;
mod client;
mod errors;
mod processor;
mod transaction;

#[derive(Parser)]
struct Cli {
    csv_transaction_file: String,
}

fn main() -> Result<()> {
    env_logger::builder().filter(None, LevelFilter::Warn).init();

    let args = Cli::parse();
    info!("csv_file_path: {}", args.csv_transaction_file);

    let mut records = BankingRecords::new(None);

    let csv_path = Path::new(&args.csv_transaction_file);

    TransactionProcessor::run_transactions_from_csv(csv_path, &mut records)?;

    println!("{}", records.clients_to_csv()?);
    Ok(())
}
