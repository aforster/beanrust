use jiff::civil::Date;
use rust_decimal::Decimal;
use std::fmt::Display;

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
#[derive(PartialEq, Debug)]
pub struct Amount {
    pub number: Decimal,
    pub currency: String,
}

impl Amount {
    pub fn new(number: Decimal, currency: String) -> Self {
        Self { number, currency }
    }
}

#[derive(Debug)]
pub enum TransactionFlag {
    OK,
    Error,
}

#[derive(Debug)]
pub struct Transaction {
    pub date: Date,
    pub flag: TransactionFlag,
    pub payee: Option<String>,
    pub narration: Option<String>,
    pub postings: Vec<Posting>,
}

// Cost represents the cost at which an asset was acquired.
// E.g. 500 META {30 USD} means that 500 shares of META was acquired at a cost of 30 USD.

#[derive(Debug)]
pub struct Cost {
    pub amount: Amount,
}

#[derive(Debug)]
pub enum CostType {
    Knwon(Cost),
    Automatic,
}

// Price paid or received for an asset. E.g. 500 USD @ 1.2CHF means that 500 USD was
// bought or sold at a price of 1.2 CHF per USD.
// 500 META {30 USD} @ 50 USD means that 500 shares of META with a cost of 30 USD was
// bought or sold (very likely sold for that syntax) at a price of 50 USD per META share.
#[derive(Debug)]
pub struct Price {
    pub date: Date,
    // Price for currency
    pub currency: String,
    // Price in amount
    pub amount: Amount,
}

#[derive(Debug)]
pub struct Posting {
    pub account: String,
    pub amount: Amount,
    pub price: Option<Price>,
    // If the cost type is automatic, then the cost will be determined once
    // all transactions were parsed. Then the appropriate lot will be found
    // to determine the actual cost.
    // TODO: Is an enum this deep really a good idea? Or should we have
    // different Transaction types before and after finishing parsing?
    pub cost: Option<CostType>,
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

impl Transaction {
    // Verify that the sum of all amounts in postings is zero.
    pub fn check(&self) -> Result<(), String> {
        if self.postings.is_empty() {
            return Ok(());
        }
        let sum = sum_amounts_it(self.postings.iter().map(|p| &p.amount)).map_err(|x| {
            format!("Invalid collection of amounts in postings: Error {x}. Transaction: {self}")
        })?;
        if sum.number != Decimal::new(0, 0) {
            return Err(format!("Transaction not balanced: total is {sum}"));
        }
        Ok(())
    }
}

impl Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.number, self.currency)
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // temporary implementation until we have a printer implementation.
        write!(f, "{:#?}", self)
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
    use jiff::civil::date;
    use std::str::FromStr;

    fn num(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn test_sum_amounts() {
        assert!(sum_amounts_it([].iter()).is_err());
        assert_eq!(
            sum_amounts_it(
                [
                    Amount::new(num("100"), "USD".to_string()),
                    Amount::new(num("-50"), "USD".to_string())
                ]
                .iter()
            )
            .unwrap(),
            Amount::new(num("50"), "USD".to_string())
        );
        assert_eq!(
            sum_amounts_it([Amount::new(num("-50"), "USD".to_string())].iter()).unwrap(),
            Amount::new(num("-50"), "USD".to_string())
        );

        assert!(
            sum_amounts_it(
                [
                    Amount::new(num("100"), "USD".to_string()),
                    Amount::new(num("-50"), "CHF".to_string())
                ]
                .iter()
            )
            .is_err()
        );
    }

    #[test]
    fn test_transaction_check() {
        let mut t = Transaction {
            date: date(2023, 1, 1),
            flag: TransactionFlag::OK,
            payee: None,
            narration: None,
            postings: vec![],
        };
        assert!(t.check().is_ok());
        let account = "Assets:Cash".to_string();
        t.postings.push(Posting {
            account: account.clone(),
            amount: Amount::new(num("100"), "USD".to_string()),
            price: None,
            cost: None,
        });
        assert!(t.check().is_err());
        t.postings.push(Posting {
            account,
            amount: Amount::new(num("-100"), "USD".to_string()),
            price: None,
            cost: None,
        });
        assert!(t.check().is_ok());
    }
}
