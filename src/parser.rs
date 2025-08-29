mod statement_iterator;

use crate::core::types::*;
use error::ParseError;
use jiff::civil::Date;
use rust_decimal::Decimal;
use std::error::Error;
use std::{fs, path::Path, str::FromStr, vec};

pub struct ParsedEntries {
    pub open: Vec<Open>,
    pub balance: Vec<Balance>,
    pub close: Vec<Close>,
    pub commodity: Vec<Commodity>,
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
            return data;
        }
        if is_comment_char(c) {
            // found a comment char, trim the string here.
            return &data[..i];
        }
    }
    data
}

/// input is a complete entry as a string, it can be multiple lines for eg transactions.

struct StatementParser<'a> {
    statement: &'a str, // complete statement, can be multiline
    remaining: &'a str, // remaining, unparsed statement.
}

impl<'a> StatementParser<'a> {
    pub fn new(statement: &'a str) -> Self {
        StatementParser {
            statement,
            remaining: statement,
        }
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
        self.remaining = trim_comment_at_end(remain).trim();
        match cmd {
            "open" => Ok(EntryVariant::Open(self.parse_open(date)?)),
            "close" => Ok(EntryVariant::Close(self.parse_close(date)?)),
            "balance" => Ok(EntryVariant::Balance(self.parse_balance(date)?)),
            "commodity" => Ok(EntryVariant::Commodity(self.parse_commodity(date)?)),
            &_ => Err(self.new_parse_err(format!("Unknown command `{}` in entry", cmd))),
        }
    }

    fn new_parse_err(&self, context: String) -> Box<ParseError> {
        Box::new(ParseError {
            context,
            failed_statement: self.statement.to_string(),
        })
    }

    fn parse_account(
        &self,
        allow_remaining: bool,
    ) -> Result<(String, Option<&'a str>), Box<ParseError>> {
        let input = self.remaining.trim();
        match input.split_once(" ") {
            None => {
                if input.is_empty() {
                    Err(self.new_parse_err("No account specified in entry".to_string()))
                } else {
                    Ok((input.to_string(), None))
                }
            }
            Some((acc, remaining)) => {
                if !allow_remaining && !remaining.is_empty() {
                    return Err(self.new_parse_err(format!(
                        "Unexpected remaining input in account parsing: `{}`",
                        remaining
                    )));
                }
                Ok((acc.trim().to_string(), Some(remaining)))
            }
        }
    }

    /// The parse functions returning entry types do not have to update self.remaining, as the parser is done after this.
    fn parse_open(&self, date: Date) -> Result<Open, Box<ParseError>> {
        let (account, remaining) = self.parse_account(true)?;

        // handle allowed currencies here.
        let mut allowed_currencies = None;
        if let Some(remaining) = remaining {
            let mut currencies = Vec::new();
            for currency in remaining.split(" ") {
                currencies.push(currency.trim().to_string());
            }
            allowed_currencies = Some(currencies);
        }
        Ok(Open {
            date,
            account,
            allowed_currencies,
        })
    }

    fn parse_close(&self, date: Date) -> Result<Close, Box<ParseError>> {
        let account = self.parse_account(false)?.0;

        Ok(Close { date, account })
    }

    fn parse_commodity(&self, date: Date) -> Result<Commodity, Box<ParseError>> {
        let commodity = self.remaining.trim();
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

    fn parse_balance(&self, date: Date) -> Result<Balance, Box<ParseError>> {
        let (account, remaining) = self.parse_account(true)?;
        let (amnt_string, currency) = remaining
            .ok_or(self.new_parse_err("No amount in balance entry".to_string()))?
            .trim()
            .split_once(' ')
            .ok_or(self.new_parse_err("no currency in balance entry".to_string()))?;

        let number = Decimal::from_str_exact(amnt_string).map_err(|e| {
            self.new_parse_err(format!(
                "unable to parse amount number in balance entry: {}",
                e
            ))
        })?;
        let currency = currency.trim();
        if currency.contains(' ') {
            return Err(self.new_parse_err(format!(
                "unexpected remaining input in currency parsing: `{}`",
                currency
            )));
        }
        Ok(Balance {
            date,
            account,
            amount: Amount::new(number, currency.to_string()),
        })
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
        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot:META META",
        }
        .parse_open(date(2022, 1, 1))
        .unwrap();
        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot:META");
        assert_eq!(entry.allowed_currencies, Some(vec!["META".to_string()]));

        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot:Cash",
        }
        .parse_open(date(2022, 2, 1))
        .unwrap();

        assert_eq!(entry.date, date(2022, 2, 1));
        assert_eq!(entry.account, "Assets:Depot:Cash");
        assert_eq!(entry.allowed_currencies, None);

        Ok(())
    }

    #[test]
    fn test_parse_close() -> Result<(), String> {
        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot:META  ",
        }
        .parse_close(date(2022, 1, 1))
        .unwrap();

        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot:META");

        Ok(())
    }

    #[test]
    fn test_parse_balance() -> Result<(), String> {
        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot:META 5 CHF ",
        }
        .parse_balance(date(2022, 1, 1))
        .unwrap();

        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot:META");
        assert_eq!(entry.amount.number, Decimal::new(5, 0));
        assert_eq!(entry.amount.currency, "CHF");

        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot -5.123456 CHF",
        }
        .parse_balance(date(2022, 1, 1))
        .unwrap();

        assert_eq!(entry.date, date(2022, 1, 1));
        assert_eq!(entry.account, "Assets:Depot");
        assert_eq!(entry.amount.number, Decimal::new(-5123456, 6));
        assert_eq!(entry.amount.currency, "CHF");

        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot  ",
        }
        .parse_balance(date(2022, 1, 1));
        assert!(entry.is_err());

        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot 3 ",
        }
        .parse_balance(date(2022, 1, 1));
        assert!(entry.is_err());

        let entry = StatementParser {
            statement: "",
            remaining: "Assets:Depot usd chf ",
        }
        .parse_balance(date(2022, 1, 1));
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
    fn test_parse_account() -> Result<(), String> {
        let (acc, rem) = StatementParser {
            statement: "",
            remaining: "Assets:Depot:META META",
        }
        .parse_account(true)
        .unwrap();
        assert_eq!(acc, "Assets:Depot:META".to_string());
        assert_eq!(rem, Some("META"));
        let result = StatementParser {
            statement: "",
            remaining: "Assets:Depot:META META",
        }
        .parse_account(false);
        assert!(result.is_err());

        let (acc, rem) = StatementParser {
            statement: "",
            remaining: "Assets:Depot:Cash",
        }
        .parse_account(true)
        .unwrap();
        assert_eq!(acc, "Assets:Depot:Cash".to_string());
        assert_eq!(rem, None);
        let (acc, rem) = StatementParser {
            statement: "",
            remaining: "Assets:Depot:Cash",
        }
        .parse_account(false)
        .unwrap();
        assert_eq!(acc, "Assets:Depot:Cash".to_string());
        assert_eq!(rem, None);

        let result = StatementParser {
            statement: "",
            remaining: "",
        }
        .parse_account(false);
        assert!(result.is_err());

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
