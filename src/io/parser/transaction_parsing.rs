use jiff::civil::Date;

use super::date_and_cmd;
use crate::{core::types::*, io::parser::TokenIterator};

impl TryFrom<&str> for Transaction {
    type Error = String;
    fn try_from(statement: &str) -> Result<Self, Self::Error> {
        let (date, flag, remain) = date_and_cmd(statement)?;

        Transaction::try_from((
            date,
            parse_flag(flag).ok_or(format!("Invalid flag: {flag}"))?,
            remain,
        ))
    }
}

fn parse_narration_and_payee(header: &str) -> Result<(Option<String>, Option<String>), String> {
    let mut first = None;
    let mut second = None;
    for token in TokenIterator::new(header) {
        if !token.starts_with('"') && !token.ends_with('"') {
            return Err(format!(
                "Invalid transaction header: {header}. Narration/payee must be quoted"
            ));
        }
        let trimmed = token.trim_matches('"');
        if first.is_none() {
            first = Some(trimmed.to_string());
        } else if second.is_none() {
            second = Some(trimmed.to_string());
        } else {
            return Err(format!(
                "Too many quoted strings in transaction header: {header}"
            ));
        }
    }
    if second.is_some() {
        Ok((first, second))
    } else {
        Ok((None, first))
    }
}

impl TryFrom<&str> for Posting {
    type Error = String;
    fn try_from(input: &str) -> Result<Self, Self::Error> {
        // Format is <account> <amount> [@|@@ <price>] [{<cost>}|{{<cost>}}]
        let (acc, remain) = input
            .split_once(' ')
            .ok_or(format!("No account in posting: {input}"))?;
        let remain = remain.trim();
        let nr_end = remain
            .find(' ')
            .ok_or(format!("No valid amount in posting: {input}"))?;
        let currency_start = nr_end
            + remain[nr_end..]
                .find(|c: char| !c.is_whitespace())
                .ok_or(format!("No valid amount in posting: {input}"))?;
        let amnt_end = currency_start
            + remain[currency_start..]
                .find(|c: char| c.is_whitespace())
                .unwrap_or(remain.len() - currency_start);

        let amount: Amount = remain[..amnt_end].trim().try_into()?;
        let remain = remain[amnt_end..].trim();
        if remain.is_empty() {
            return Ok(Posting {
                account: acc.to_string(),
                amount,
                price: None,
                cost: None,
            });
        }

        Err("not implemented".to_string())
    }
}

impl TryFrom<(Date, TransactionFlag, &str)> for Transaction {
    type Error = String;
    fn try_from(input: (Date, TransactionFlag, &str)) -> Result<Self, Self::Error> {
        let (date, flag, statement) = input;
        let (header, postings_str) = statement.split_once('\n').unwrap_or((statement, ""));
        let (payee, narration) = parse_narration_and_payee(header.trim())?;

        // Parse postings:
        let mut postings: Vec<Posting> = vec![];
        for line in postings_str.lines() {
            let posting = Posting::try_from(line.trim())
                .map_err(|e| format!("Unable to parse posting '{line}': {e}"))?;
            postings.push(posting);
        }

        Ok(Transaction {
            date,
            flag,
            payee,
            narration,
            postings,
        })
    }
}

pub fn parse_flag(s: &str) -> Option<TransactionFlag> {
    match s {
        "*" => Some(TransactionFlag::OK),
        "!" => Some(TransactionFlag::Error),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;
    use rust_decimal::Decimal;

    #[test]
    fn test_tryfrom_transaction() -> Result<(), String> {
        let result = Transaction::try_from("2022-05-03 *")?;
        assert_eq!(result.postings.len(), 0);
        assert_eq!(result.date, date(2022, 5, 3));
        assert_eq!(result.flag, crate::core::types::TransactionFlag::OK);

        let result = Transaction::try_from(
            "2022-05-03 *\n    Assets:Cash 5   CHF\n    Assets:Cash2   5.1234 USD  ",
        )?;
        assert_eq!(result.postings.len(), 2);
        assert_eq!(result.postings[0].account, "Assets:Cash");
        assert_eq!(result.postings[0].amount.number, 5.into());
        assert_eq!(result.postings[0].amount.currency, "CHF");
        assert_eq!(result.postings[1].account, "Assets:Cash2");
        assert_eq!(result.postings[1].amount.number, Decimal::new(51234, 4));
        assert_eq!(result.postings[1].amount.currency, "USD");
        assert_eq!(result.date, date(2022, 5, 3));
        assert_eq!(result.flag, crate::core::types::TransactionFlag::OK);

        Ok(())
    }

    #[test]
    fn test_parse_flag() {
        assert_eq!(parse_flag("*"), Some(TransactionFlag::OK));
        assert_eq!(parse_flag("!"), Some(TransactionFlag::Error));
        assert_eq!(parse_flag("x"), None);
    }
}
