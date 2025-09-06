use super::{Amount, sum_amounts_it};
use jiff::civil::Date;
use rust_decimal::Decimal;

#[derive(Debug, PartialEq)]
pub enum TransactionFlag {
    OK,
    Error,
}

// Cost represents the cost at which an asset was acquired.
// E.g. 500 META {30 USD} means that 500 shares of META was acquired at a cost of 30 USD.

#[derive(Debug)]
pub struct Cost {
    pub amount: Amount,
}

#[derive(Debug)]
pub enum CostType {
    Known(Cost),
    Automatic,
}

// Price paid or received for an asset. E.g. 500 USD @ 1.2CHF means that 500 USD was
// bought or sold at a price of 1.2 CHF per USD.
// 500 META {30 USD} @ 50 USD means that 500 shares of META with a cost of 30 USD was
// bought or sold (very likely sold for that syntax) at a price of 50 USD per META share.
#[derive(Debug)]
pub struct Price {
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

#[derive(Debug)]
pub struct Transaction {
    pub date: Date,
    pub flag: TransactionFlag,
    pub payee: Option<String>,
    pub narration: Option<String>,
    pub postings: Vec<Posting>,
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

#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;

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
            amount: Amount::new(100.into(), "USD".to_string()),
            price: None,
            cost: None,
        });
        assert!(t.check().is_err());
        t.postings.push(Posting {
            account,
            amount: Amount::new((-100).into(), "USD".to_string()),
            price: None,
            cost: None,
        });
        assert!(t.check().is_ok());
    }
}
