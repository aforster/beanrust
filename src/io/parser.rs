mod statement_iterator;
mod transaction_parsing;

use crate::core::types::*;
use error::ParseError;
use jiff::civil::Date;
use rust_decimal::Decimal;
pub use statement_iterator::TokenIterator;
use std::error::Error;
use std::{fs, path::Path, str::FromStr, vec};

pub struct ParsedEntries {
    pub open: Vec<Open>,
    pub balance: Vec<Balance>,
    pub close: Vec<Close>,
    pub commodity: Vec<Commodity>,
    pub price: Vec<PriceEntry>,
    // temporry until impl complete
    pub unhandled_entries: Vec<String>,
}

impl ParsedEntries {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn len(&self) -> usize {
        self.open.len() + self.balance.len() + self.close.len()
    }
    pub fn push(&mut self, entry: EntryVariant) {
        match entry {
            EntryVariant::Open(o) => self.open.push(o),
            EntryVariant::Balance(b) => self.balance.push(b),
            EntryVariant::Close(c) => self.close.push(c),
            EntryVariant::Commodity(c) => self.commodity.push(c),
            EntryVariant::PriceEntry(p) => self.price.push(p),
            _ => {
                panic!("Unsupported entry type in push")
            }
        }
    }
    pub fn push_result(&mut self, entry: Result<EntryVariant, Box<error::ParseError>>) {
        match entry {
            Ok(e) => self.push(e),
            Err(e) => {
                self.unhandled_entries.push(e.failed_statement);
            }
        }
    }
}

impl Default for ParsedEntries {
    fn default() -> Self {
        ParsedEntries {
            open: vec![],
            balance: vec![],
            close: vec![],
            commodity: vec![],
            price: vec![],
            unhandled_entries: vec![],
        }
    }
}

pub fn parse_entries_from_file(fpath: &Path) -> Result<ParsedEntries, Box<dyn Error>> {
    parse_entries_from_string(fs::read_to_string(fpath)?, fpath)
}

pub fn parse_entries_from_string(
    input: String,
    _cur_fpath: &Path,
) -> Result<ParsedEntries, Box<dyn Error>> {
    // TODO: Handle imports of other files.
    let mut parsed_entries: ParsedEntries = ParsedEntries::default();

    statement_iterator::StatementIterator::new(&input)
        .map(|s| StatementParser::new(s).parse_entry())
        .for_each(|r| {
            // todo: don't swallow errors here.
            parsed_entries.push_result(r);
        });

    Ok(parsed_entries)
}

pub fn is_comment_char(c: char) -> bool {
    c == ';' || c == '#'
}

fn trim_comment_at_end(data: &str) -> &str {
    for (i, c) in data.char_indices().rev() {
        // if we find a newline, then we are done. We can only trim comments on the last line.
        if c == '\n' {
            break;
        }
        if is_comment_char(c) {
            // found a comment char, trim the string here.
            return &data[..i].trim();
        }
    }
    data.trim()
}

/// input is a complete entry as a string, it can be multiple lines for eg transactions.

struct StatementParser<'a> {
    statement: &'a str, // complete statement, can be multiline
}

impl<'a> StatementParser<'a> {
    pub fn new(statement: &'a str) -> Self {
        StatementParser { statement }
    }

    pub fn parse_entry(&mut self) -> Result<EntryVariant, Box<ParseError>> {
        // An entry always starts with a date:
        let (date, remain) = self.statement.trim().split_once(" ").unwrap();
        let date: Date = Date::from_str(date)
            .map_err(|e| self.new_parse_err(format!("unable to parse date: {}", e)))?;

        let (cmd, remain) = remain
            .trim()
            .split_once(" ")
            .ok_or(self.new_parse_err("No command in entry".to_string()))?;
        let remaining = trim_comment_at_end(remain).trim();
        match cmd {
            // TODO: Change all of these to use TryFrom instead of parse_xxx functions.
            "open" => Ok(EntryVariant::Open(self.parse_open(date, remaining)?)),
            "close" => Ok(EntryVariant::Close(self.parse_close(date, remaining)?)),
            "balance" => Ok(EntryVariant::Balance(self.parse_balance(date, remaining)?)),
            "commodity" => Ok(EntryVariant::Commodity(
                self.parse_commodity(date, remaining)?,
            )),
            "price" => Ok(EntryVariant::PriceEntry(self.parse_price(date, remaining)?)),
            "*" => Ok(EntryVariant::Transaction(
                self.parse_transaction(&self.statement)?,
            )),
            "!" => Ok(EntryVariant::Transaction(
                self.parse_transaction(&self.statement)?,
            )),
            &_ => Err(self.new_parse_err(format!("Unknown command `{}` in entry", cmd))),
        }
    }

    fn new_parse_err(&self, context: String) -> Box<ParseError> {
        Box::new(ParseError {
            context,
            failed_statement: self.statement.to_string(),
        })
    }

    fn get_next_token(
        &self,
        token_it: &mut TokenIterator<'a>,
        token_type: &str,
    ) -> Result<&'a str, Box<ParseError>> {
        let next = token_it
            .next()
            .ok_or(self.new_parse_err(format!("No {token_type} found")))?;
        Ok(next)
    }
    fn err_if_more_tokens(
        &self,
        token_it: &mut TokenIterator<'a>,
        token_type: &str,
    ) -> Result<(), Box<ParseError>> {
        if let Some(_) = token_it.next() {
            return Err(self.new_parse_err(format!(
                "Unexpected remaining input in {token_type} parsing: `{}`",
                token_it.collect::<Vec<&str>>().join(" ")
            )));
        }
        Ok(())
    }

    /// The parse functions returning entry types do not have to update self.remaining, as the parser is done after this.
    fn parse_open(&self, date: Date, remaining: &str) -> Result<Open, Box<ParseError>> {
        let mut it = TokenIterator::new(remaining);
        let account = self.get_next_token(&mut it, "account")?.to_string();
        let allowed_currencies: Vec<String> = it.map(|s| s.to_string()).collect();

        Ok(Open {
            date,
            account,
            allowed_currencies: if allowed_currencies.is_empty() {
                None
            } else {
                Some(allowed_currencies)
            },
        })
    }

    fn parse_close(&self, date: Date, remaining: &str) -> Result<Close, Box<ParseError>> {
        let mut it = TokenIterator::new(remaining);
        let account = self.get_next_token(&mut it, "close")?.to_string();
        self.err_if_more_tokens(&mut it, "close")?;
        Ok(Close { date, account })
    }

    fn parse_commodity(&self, date: Date, remaining: &str) -> Result<Commodity, Box<ParseError>> {
        let commodity = remaining.trim();
        if commodity.is_empty() {
            return Err(self.new_parse_err("No commodity specified in entry".to_string()));
        }
        if commodity.contains(' ') {
            return Err(self.new_parse_err(format!(
                "unexpected remaining input in commodity parsing: `{}`",
                commodity
            )));
        }
        Ok(Commodity {
            date,
            currency: commodity.to_string(),
        })
    }

    // e.g. a statement like "Assets:Depot:META 1.23 CHF" or "META 1.23 USD"
    fn parse_str_and_price(
        &self,
        remaining: &str,
        token_type: &str,
    ) -> Result<(String, Amount), Box<ParseError>> {
        let mut it = TokenIterator::new(remaining);
        let out_str = self.get_next_token(&mut it, token_type)?.to_string();
        let amnt_string = self.get_next_token(&mut it, "amount")?;
        let currency = self.get_next_token(&mut it, "currency")?;
        self.err_if_more_tokens(&mut it, token_type)?;

        let number = Decimal::from_str_exact(amnt_string).map_err(|e| {
            self.new_parse_err(format!(
                "unable to parse amount number in {token_type} entry: {e}"
            ))
        })?;

        Ok((out_str, Amount::new(number, currency.to_string())))
    }

    fn parse_balance(&self, date: Date, remaining: &str) -> Result<Balance, Box<ParseError>> {
        let (account, amount) = self.parse_str_and_price(remaining, "balance")?;
        Ok(Balance {
            date,
            account,
            amount,
        })
    }

    // 2024-10-03 price META 1.23 CHF
    fn parse_price(&self, date: Date, remaining: &str) -> Result<PriceEntry, Box<ParseError>> {
        let (currency, amount) = self.parse_str_and_price(remaining, "price")?;
        Ok(PriceEntry {
            date,
            currency,
            amount,
        })
    }

    fn parse_transaction(&self, statement: &str) -> Result<Transaction, Box<ParseError>> {
        Transaction::try_from(statement)
            .map_err(|e| self.new_parse_err(format!("unable to parse transaction: {e}")))
    }
}

pub mod error {
    #[derive(Debug)]
    pub struct ParseError {
        pub context: String,
        pub failed_statement: String,
    }

    impl std::fmt::Display for ParseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            return write!(
                f,
                "Failed to parse ({}): `{}`",
                self.context, self.failed_statement
            );
        }
    }
    impl std::error::Error for ParseError {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn test_parse_entry() -> Result<(), String> {
        let entry = StatementParser::new("2024-01-01 open Assets:Depot:META META")
            .parse_entry()
            .unwrap();
        let entry = match entry {
            EntryVariant::Open(o) => Ok(o),
            _ => Err("Incorrect return"),
        }
        .unwrap();
        assert_eq!(entry.date, date(2024, 1, 1));
        assert_eq!(entry.account, "Assets:Depot:META");
        assert_eq!(entry.allowed_currencies, Some(vec!["META".to_string()]));
        Ok(())
    }

    #[test]
    fn test_parse_with_comment_at_end() -> Result<(), String> {
        let entry = StatementParser::new("2024-01-01 close Assets:Depot ; some comment here * * ")
            .parse_entry()
            .unwrap();
        let entry = match entry {
            EntryVariant::Close(o) => Ok(o),
            _ => Err("Incorrect return"),
        }
        .unwrap();
        assert_eq!(entry.date, date(2024, 1, 1));
        assert_eq!(entry.account, "Assets:Depot");
        Ok(())
    }

    #[test]
    fn test_parse_open() -> Result<(), String> {
        let entry = StatementParser { statement: "" }
            .parse_open(date(2022, 1, 1), "Assets:Depot:META META")
            .unwrap();
        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot:META");
        assert_eq!(entry.allowed_currencies, Some(vec!["META".to_string()]));

        let entry = StatementParser { statement: "" }
            .parse_open(date(2022, 2, 1), "Assets:Depot:Cash")
            .unwrap();

        assert_eq!(entry.date, date(2022, 2, 1));
        assert_eq!(entry.account, "Assets:Depot:Cash");
        assert_eq!(entry.allowed_currencies, None);

        Ok(())
    }

    #[test]
    fn test_parse_close() -> Result<(), String> {
        let entry = StatementParser { statement: "" }
            .parse_close(date(2022, 1, 1), "Assets:Depot:META  ")
            .unwrap();

        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot:META");

        Ok(())
    }

    #[test]
    fn test_parse_balance() -> Result<(), String> {
        let entry = StatementParser { statement: "" }
            .parse_balance(date(2022, 1, 1), "Assets:Depot:META 5 CHF ")
            .unwrap();

        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot:META");
        assert_eq!(entry.amount.number, Decimal::new(5, 0));
        assert_eq!(entry.amount.currency, "CHF");

        let entry = StatementParser { statement: "" }
            .parse_balance(date(2022, 1, 1), "Assets:Depot -5.123456 CHF")
            .unwrap();

        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot");
        assert_eq!(entry.amount.number, Decimal::new(-5123456, 6));
        assert_eq!(entry.amount.currency, "CHF");

        let entry =
            StatementParser { statement: "" }.parse_balance(date(2022, 1, 1), "Assets:Depot  ");
        assert!(entry.is_err());

        let entry =
            StatementParser { statement: "" }.parse_balance(date(2022, 1, 1), "Assets:Depot 3 ");
        assert!(entry.is_err());

        let entry = StatementParser { statement: "" }
            .parse_balance(date(2022, 1, 1), "Assets:Depot usd chf ");
        assert!(entry.is_err());

        let entry = StatementParser::new("2024-10-03   balance Assets:Depot:Cash 0 CHF")
            .parse_entry()
            .unwrap();
        let entry = match entry {
            EntryVariant::Balance(o) => Ok(o),
            _ => Err("Incorrect return"),
        }
        .unwrap();
        assert_eq!(entry.date, date(2024, 10, 3));
        assert_eq!(entry.account, "Assets:Depot:Cash");
        assert_eq!(entry.amount.number, Decimal::new(0, 0));
        assert_eq!(entry.amount.currency, "CHF");

        Ok(())
    }

    #[test]
    fn test_parsed_entries() -> Result<(), String> {
        let mut entries = ParsedEntries::default();
        assert!(entries.is_empty());
        assert_eq!(entries.len(), 0);
        entries.open.push(Open {
            date: date(2024, 1, 1),
            account: "Assets:Cash".to_string(),
            allowed_currencies: None,
        });
        assert!(!entries.is_empty());
        assert_eq!(entries.len(), 1);
        Ok(())
    }
}
