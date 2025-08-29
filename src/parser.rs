use crate::core::types::*;
use jiff::civil::{Date, date};
use rust_decimal::dec;
use std::error::Error;
use std::{fs, str::FromStr, vec};
#[derive(Debug)]
pub struct ParseError {
    pub msg: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.msg);
    }
}
impl std::error::Error for ParseError {}

fn new_parse_err(msg: String) -> Box<ParseError> {
    Box::new(ParseError { msg: msg })
}

pub fn parse_entries_from_file(fpath: &std::path::Path) -> Result<Vec<EntryVariant>, String> {
    parse_entries_from_string(fs::read_to_string(fpath).map_err(|e| format!("{}", e))?)
}

pub fn parse_entries_from_string(_input: String) -> Result<Vec<EntryVariant>, String> {
    Ok(vec![
        EntryVariant::Balance(Balance {
            date: date(2025, 1, 1),
            account: "foo".to_string(),
            amount: Amount::new(dec!(1.2), "CHF".to_string()),
        }),
        EntryVariant::Open(Open {
            date: date(2025, 1, 1),
            account: "foo".to_string(),
            allowed_currencies: None,
        }),
    ])
}

pub fn parse_entry_from_string(input: &str) -> Result<EntryVariant, Box<dyn Error>> {
    // An entry always starts with a date:
    let (date, input) = input.trim().split_once(" ").unwrap();
    let date: Date = Date::from_str(date)?;

    let (cmd, input) = input
        .split_once(" ")
        .ok_or(new_parse_err(format!("No command in entry: {}", input)))?;
    match cmd {
        "open" => parse_open(date, input),
        &_ => Err(new_parse_err(format!(
            "Unknown command in entry: {}",
            input
        ))),
    }
}

fn parse_open(date: Date, input: &str) -> Result<EntryVariant, Box<dyn Error>> {
    let mut iter = input.split(" ");
    let account = iter
        .next()
        .ok_or(new_parse_err(
            "unable to parse open statement. No account specified".to_string(),
        ))?
        .trim()
        .to_string();

    // handle allowed currencies here.
    let mut allowed_currencies = Vec::new();
    for currency in iter {
        allowed_currencies.push(currency.trim().to_string());
    }

    Ok(EntryVariant::Open(Open {
        date,
        account,
        allowed_currencies: if allowed_currencies.is_empty() {
            None
        } else {
            Some(allowed_currencies)
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entry_from_string() -> Result<(), String> {
        let entry = parse_entry_from_string("2024-01-01 open Assets:Depot:META META").unwrap();
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
}
