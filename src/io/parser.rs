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
    pub transactions: Vec<Transaction>,
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
            EntryVariant::Transaction(t) => self.transactions.push(t),
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
            transactions: vec![],
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
            return &data[..i];
        }
    }
    data
}

fn date_and_cmd<'a>(statement: &'a str) -> Result<(Date, &'a str, &'a str), String> {
    let (date, remain) = statement
        .trim_start()
        .split_once(' ')
        .ok_or(format!("No date in entry: {statement}"))?;
    let date: Date = Date::from_str(date).map_err(|e| e.to_string())?;

    let cmd;
    let remain_final;
    let remain = remain.trim_start();
    match remain.find(|c: char| c.is_whitespace()) {
        None => {
            cmd = remain.trim();
            remain_final = "";
        }
        Some(idx) => {
            (cmd, remain_final) = remain.split_at(idx);
        }
    }

    if cmd.is_empty() {
        return Err(format!("No command in entry: {statement}"));
    }
    Ok((date, cmd, remain_final))
}

fn consume_amount(input: &str) -> Result<(Amount, &str), String> {
    // Options are <number> <currency> or <number><currency>. In the future maybe also  <math><currency>
    // currencies must start with a letter, so lets search for the first character which is a letter,
    // The number definitely won't contain a letter...
    let currency_start = input
        .find(|c: char| c.is_alphabetic())
        .ok_or(format!("No currency found: {input}"))?;
    let currency_end = currency_start
        + input[currency_start..]
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(input.len() - currency_start);
    let (amount_str, remain) = input.split_at(currency_end);
    Ok((amount_str.try_into()?, remain))
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
        let (date, cmd, remain) =
            date_and_cmd(self.statement).map_err(|e| self.new_parse_err(e))?;
        let remaining = trim_comment_at_end(remain);
        if let Some(flag) = transaction_parsing::parse_flag(cmd) {
            // This is a transaction entry, the rest of the statement is the complete transaction.
            return Ok(EntryVariant::Transaction(
                self.parse_transaction(date, flag, remaining)?,
            ));
        }
        match cmd {
            // TODO: Change all of these to use TryFrom instead of parse_xxx functions.
            "open" => Ok(EntryVariant::Open(self.parse_open(date, remaining)?)),
            "close" => Ok(EntryVariant::Close(self.parse_close(date, remaining)?)),
            "balance" => Ok(EntryVariant::Balance(self.parse_balance(date, remaining)?)),
            "commodity" => Ok(EntryVariant::Commodity(
                self.parse_commodity(date, remaining)?,
            )),
            "price" => Ok(EntryVariant::PriceEntry(self.parse_price(date, remaining)?)),

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

    fn parse_transaction(
        &self,
        date: Date,
        flag: TransactionFlag,
        statement: &str,
    ) -> Result<Transaction, Box<ParseError>> {
        Transaction::try_from((date, flag, statement))
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
    fn test_consume_amount() {
        let (amnt, remain) = consume_amount("5 CHF some remaining").unwrap();
        assert_eq!(amnt.number, Decimal::new(5, 0));
        assert_eq!(amnt.currency, "CHF");
        assert_eq!(remain, " some remaining");

        let (amnt, remain) = consume_amount("-5.1234 USD some remaining").unwrap();
        assert_eq!(amnt.number, Decimal::new(-51234, 4));
        assert_eq!(amnt.currency, "USD");
        assert_eq!(remain, " some remaining");

        let (amnt, remain) = consume_amount("5 BTC").unwrap();
        assert_eq!(amnt.number, Decimal::new(5, 0));
        assert_eq!(amnt.currency, "BTC");
        assert_eq!(remain, "");

        let (amnt, remain) = consume_amount("5BTC").unwrap();
        assert_eq!(amnt.number, Decimal::new(5, 0));
        assert_eq!(amnt.currency, "BTC");
        assert_eq!(remain, "");

        assert!(consume_amount("5").is_err());
        assert!(consume_amount("CHF").is_err());
        assert!(consume_amount("CHF 5").is_err());
        assert!(consume_amount("5,67 CHF").is_err());
    }

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

    #[test]
    fn test_date_and_cmd() {
        let (d, cmd, remain) = date_and_cmd("2024-01-01 open Assets:Cash").unwrap();
        assert_eq!(d, date(2024, 1, 1));
        assert_eq!(cmd, "open");
        assert_eq!(remain, " Assets:Cash");

        let (d, cmd, remain) = date_and_cmd("2024-01-01    open   Assets:Cash   ").unwrap();
        assert_eq!(d, date(2024, 1, 1));
        assert_eq!(cmd, "open");
        assert_eq!(remain, "   Assets:Cash   ");

        let (d, cmd, remain) = date_and_cmd("2024-01-01 open").unwrap();
        assert_eq!(d, date(2024, 1, 1));
        assert_eq!(cmd, "open");
        assert_eq!(remain, "");

        let (d, cmd, remain) = date_and_cmd("2024-01-01 *\n").unwrap();
        assert_eq!(d, date(2024, 1, 1));
        assert_eq!(cmd, "*");
        assert_eq!(remain, "\n");

        let (d, cmd, remain) = date_and_cmd("2024-01-01 * \n fo bar").unwrap();
        assert_eq!(d, date(2024, 1, 1));
        assert_eq!(cmd, "*");
        assert_eq!(remain, " \n fo bar");

        let res = date_and_cmd("2024-01-01");
        assert!(res.is_err());

        let res = date_and_cmd("open Assets:Cash");
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_transaction() {
        let mut entry = StatementParser::new(
            "2024-10-05 *
  Assets:Depot:Cash   -100 CHF
  Assets:Depot:AMD      1 AMD {100 CHF}  ",
        );
        let parsed = entry.parse_entry().unwrap();
        let t = match parsed {
            EntryVariant::Transaction(o) => Ok(o),
            _ => Err("Incorrect return"),
        }
        .unwrap();
        assert_eq!(t.date, date(2024, 10, 5));
        assert_eq!(t.flag, TransactionFlag::OK);
        assert_eq!(t.postings.len(), 2);
        assert_eq!(t.postings[0].account, "Assets:Depot:Cash");
        assert_eq!(t.postings[0].amount.number, Decimal::new(-100, 0));
        assert_eq!(t.postings[0].amount.currency, "CHF");
        assert!(t.postings[0].price.is_none());
        assert!(t.postings[0].cost.is_none());
        assert_eq!(t.postings[1].account, "Assets:Depot:AMD");
        assert_eq!(t.postings[1].amount.number, Decimal::new(1, 0));
        assert_eq!(t.postings[1].amount.currency, "AMD");
        assert!(t.postings[1].price.is_none());
        assert!(t.postings[1].cost.is_some());
        if let Some(CostType::Known(c)) = &t.postings[1].cost {
            assert_eq!(c.amount.number, Decimal::new(100, 0));
            assert_eq!(c.amount.currency, "CHF");
        } else {
            panic!("Cost not parsed correctly");
        }
    }
}
