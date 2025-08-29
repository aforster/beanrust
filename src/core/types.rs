use jiff::civil::Date;
use rust_decimal::Decimal;
pub enum EntryVariant {
    Transaction(Transaction),
    Balance(Balance),
    Open(Open),
    Close(Close),
}

impl EntryVariant {
    pub fn date(&self) -> Date {
        match self {
            EntryVariant::Transaction(t) => t.date,
            EntryVariant::Balance(t) => t.date,
            EntryVariant::Open(t) => t.date,
            EntryVariant::Close(t) => t.date,
        }
    }
}

pub struct Amount {
    pub number: Decimal,
    pub currency: String,
}

impl Amount {
    pub fn new(number: Decimal, currency: String) -> Self {
        Self { number, currency }
    }
}

pub struct Transaction {
    pub date: Date,
}

pub struct Balance {
    pub date: Date,
    pub account: String,
    pub amount: Amount,
}

pub struct Open {
    pub date: Date,
    pub account: String,
    pub allowed_currencies: Option<Vec<String>>,
}
pub struct Close {
    pub date: Date,
    pub account: String,
}
