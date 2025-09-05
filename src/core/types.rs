pub mod transaction;

pub use transaction::{Cost, CostType, Posting, Price, Transaction, TransactionFlag};

use crate::io::parser::TokenIterator;
use crate::io::printer::print_transaction;
use jiff::civil::Date;
use rust_decimal::Decimal;
use std::fmt::Display;

pub enum EntryVariant {
    Transaction(Transaction),
    Balance(Balance),
    Open(Open),
    Close(Close),
    Commodity(Commodity),
    PriceEntry(PriceEntry),
}

impl EntryVariant {
    pub fn date(&self) -> Date {
        match self {
            EntryVariant::Transaction(t) => t.date,
            EntryVariant::Balance(t) => t.date,
            EntryVariant::Open(t) => t.date,
            EntryVariant::Close(t) => t.date,
            EntryVariant::Commodity(c) => c.date,
            EntryVariant::PriceEntry(p) => p.date,
        }
    }
}
#[derive(PartialEq, Debug, Clone)]
pub struct Amount {
    pub number: Decimal,
    pub currency: String,
}

impl Amount {
    pub fn new(number: Decimal, currency: String) -> Self {
        Self { number, currency }
    }
}

pub struct PriceEntry {
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

impl Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.number, self.currency)
    }
}

impl TryFrom<&str> for Amount {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut it = TokenIterator::new(value);
        let number_str = it.next().ok_or_else(|| "No number found".to_string())?;
        let number: Decimal = number_str
            .try_into()
            .map_err(|e| format!("Error parsing number '{number_str}': {e}"))?;
        let currency = it
            .next()
            .ok_or_else(|| "No currency found".to_string())?
            .to_string();
        if it.next().is_some() {
            return Err("Extra tokens found after currency".to_string());
        }
        Ok(Amount { number, currency })
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", print_transaction(self))
    }
}

fn sum_amounts_it<'a, It>(amounts: It) -> Result<Amount, String>
where
    It: Iterator<Item = &'a Amount>,
{
    let mut currency: Option<String> = None;
    let mut total = Decimal::new(0, 0);
    for a in amounts {
        if let Some(c) = &currency {
            if c != &a.currency {
                return Err(format!(
                    "Multiple currencies in given amounts: {} and {}",
                    c, a.currency
                ));
            }
        } else {
            currency = Some(a.currency.clone());
        }
        total += a.number;
    }
    Ok(Amount::new(
        total,
        currency.ok_or_else(|| "No amounts in transaction".to_string())?,
    ))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_try_amount_from_string() {
        assert_eq!(
            Amount::try_from("100     USD  ").unwrap(),
            Amount::new(100.into(), "USD".to_string())
        );
        assert_eq!(
            Amount::try_from("-0.43 USD").unwrap(),
            Amount::new(Decimal::new(-43, 2), "USD".to_string())
        );
        assert!(Amount::try_from("100").is_err());
        assert!(Amount::try_from("100 USD extra").is_err());
        assert!(Amount::try_from("abc USD").is_err());
        assert!(Amount::try_from("100  ").is_err());
    }

    #[test]
    fn test_sum_amounts() {
        assert!(sum_amounts_it([].iter()).is_err());
        assert_eq!(
            sum_amounts_it(
                [
                    Amount::new(100.into(), "USD".to_string()),
                    Amount::new((-50).into(), "USD".to_string())
                ]
                .iter()
            )
            .unwrap(),
            Amount::new(50.into(), "USD".to_string())
        );
        assert_eq!(
            sum_amounts_it([Amount::new((-50).into(), "USD".to_string())].iter()).unwrap(),
            Amount::new((-50).into(), "USD".to_string())
        );

        assert!(
            sum_amounts_it(
                [
                    Amount::new(100.into(), "USD".to_string()),
                    Amount::new((-50).into(), "CHF".to_string())
                ]
                .iter()
            )
            .is_err()
        );
    }
}
