use crate::core::types::*;
use jiff::civil::Date;
use std::error::Error;
use std::{fs, path::Path, str::FromStr, vec};

pub struct ParsedEntries {
    pub open: Vec<Open>,
    pub balance: Vec<Balance>,
    // temporry until impl complete
    pub unhandled_entries: Vec<String>,
}

impl ParsedEntries {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn len(&self) -> usize {
        self.open.len() + self.balance.len()
    }
    pub fn push(&mut self, entry: EntryVariant) {
        match entry {
            EntryVariant::Open(o) => self.open.push(o),
            EntryVariant::Balance(b) => self.balance.push(b),
            _ => {
                panic!("Unsupported entry type in push")
            }
        }
    }
    pub fn push_result(&mut self, entry: Result<EntryVariant, Box<error::ParseError>>) {
        match entry {
            Ok(e) => self.push(e),
            Err(e) => {
                self.unhandled_entries.push(e.failed_entry);
            }
        }
    }
}

impl Default for ParsedEntries {
    fn default() -> Self {
        ParsedEntries {
            open: vec![],
            balance: vec![],
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

    let new_entry_matcher = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}.*")?;
    let new_multiline_entry_matcher = regex::Regex::new(r"^.* \*.*")?;
    let mut parsed_entries = ParsedEntries::default();

    // Split input into entries. Entries either start with a date, or some specific keyword.

    let mut line_it = input.lines();
    let mut multiline_entry = String::new();
    loop {
        // todo can we do this by not creating a new string for each entry but rather refer to substrings of the original input?
        //todo check if an entry is multiline or not. if not, directly process without copy
        let line = match line_it.next() {
            Some(l) => l.trim(),
            None => break,
        };
        if line.is_empty() || line.starts_with(';') {
            // skip empty lines and comments
            continue;
        }
        if new_entry_matcher.is_match(line) {
            // Start of a new entry, proces last one.
            if !multiline_entry.is_empty() {
                parsed_entries.push_result(parse_entry(&multiline_entry));
                multiline_entry.clear();
            }
            if new_multiline_entry_matcher.is_match(line) {
                multiline_entry = line.to_string();
            } else {
                parsed_entries.push_result(parse_entry(line));
            }
        } else {
            multiline_entry.push('\n');
            multiline_entry += line;
        }
    }
    if !multiline_entry.is_empty() {
        parsed_entries.push_result(parse_entry(&multiline_entry));
    }

    Ok(parsed_entries)
}

/// input is a complete entry as a string, it can be multiple lines for eg transactions.
pub fn parse_entry(input: &str) -> Result<EntryVariant, Box<error::ParseError>> {
    // An entry always starts with a date:
    let (date, remain) = input.trim().split_once(" ").unwrap();
    let date: Date = Date::from_str(date)
        .map_err(|e| new_parse_err(input, format!("unable to parse date: {}", e)))?;

    let (cmd, remain) = remain
        .split_once(" ")
        .ok_or(new_parse_err(input, "No command in entry".to_string()))?;
    match cmd {
        "open" => parse_open(date, remain),
        &_ => Err(new_parse_err(
            input,
            format!("Unknown command `{}` in entry", cmd),
        )),
    }
}

fn new_parse_err(entry: &str, context: String) -> Box<error::ParseError> {
    Box::new(error::ParseError {
        context,
        failed_entry: entry.to_string(),
    })
}

fn parse_open(date: Date, input: &str) -> Result<EntryVariant, Box<error::ParseError>> {
    let mut iter = input.split(" ");
    let account = iter
        .next()
        .ok_or(new_parse_err(
            input,
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

pub mod error {
    #[derive(Debug)]
    pub struct ParseError {
        pub context: String,
        pub failed_entry: String,
    }

    impl std::fmt::Display for ParseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            return write!(
                f,
                "Failed to parse ({}): `{}`",
                self.context, self.failed_entry
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
        let entry = parse_entry("2024-01-01 open Assets:Depot:META META").unwrap();
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
