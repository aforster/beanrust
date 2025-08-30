use jiff::civil::Date;
use rust_decimal::Decimal;
pub enum EntryVariant {
    Transaction(Transaction),
    Balance(Balance),
    Open(Open),
    Close(Close),
    Commodity(Commodity),
    Price(Price),
}

impl EntryVariant {
    pub fn date(&self) -> Date {
        match self {
            EntryVariant::Transaction(t) => t.date,
            EntryVariant::Balance(t) => t.date,
            EntryVariant::Open(t) => t.date,
            EntryVariant::Close(t) => t.date,
            EntryVariant::Commodity(c) => c.date,
            EntryVariant::Price(p) => p.date,
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

pub struct Price {
    pub date: Date,
    // Price for currency
    pub currency: String,
    // Price in amount
    pub amount: Amount,
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

pub struct Commodity {
    pub date: Date,
    pub currency: String,
}
