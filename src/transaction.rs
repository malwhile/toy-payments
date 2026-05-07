use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::bankrecords::deserialize_amount_strict;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Transaction {
    #[serde(alias = "type")]
    pub type_: TransactionType,
    pub client: u16,
    pub tx: u32,
    #[serde(deserialize_with = "deserialize_amount_strict")]
    pub amount: Decimal,
}
