use super::{consume_amount, date_and_cmd};
use crate::{
    core::types::*,
    io::parser::{TokenIterator, trim_comment_at_end},
};
use jiff::civil::Date;
use regex::Regex;

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

impl TryFrom<&str> for Posting {
    type Error = String;
    fn try_from(input: &str) -> Result<Self, Self::Error> {
        // Format is <account> <amount> [@|@@ <price>] [{<cost>}|{{<cost>}}]
        let (acc, remain) = input
            .split_once(' ')
            .ok_or(format!("No account in posting: {input}"))?;
        let (amount, remain) = consume_amount(remain)?;
        let remain = remain.trim();
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
            let sanitized = trim_comment_at_end(line);
            if !sanitized.is_empty() {
                let posting = Posting::try_from(sanitized)
                    .map_err(|e| format!("Unable to parse posting '{line}': {e}"))?;
                postings.push(posting);
            }
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

#[derive(Debug)]
struct Parsed<T> {
    data: T,
    per_unit: bool,
}

fn parse_price_and_cost(
    input: &str,
) -> Result<(Option<Parsed<Price>>, Option<Parsed<Cost>>), String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok((None, None));
    }
    let amnt_regex = r"(\d+.*\w+)";
    let reg = Regex::new(
        &format!(r"^((\@ *(?P<unitpr>{amnt_regex}))|(\@\@ *(?P<totpr>{amnt_regex})))? *((\{{ *(?P<unitcost>{amnt_regex})\}})|(\{{\{{ *(?P<totcost>{amnt_regex} *)\}}\}}))?$",
    )).unwrap();

    let mut price = None;
    let mut cost = None;
    for capture in reg.captures_iter(input) {
        if let Some(unit_price) = capture.name("unitpr") {
            price = Some(Parsed::<Price> {
                data: Price {
                    amount: unit_price.as_str().try_into()?,
                },
                per_unit: true,
            });
        } else if let Some(tot_price) = capture.name("totpr") {
            price = Some(Parsed::<Price> {
                data: Price {
                    amount: tot_price.as_str().try_into()?,
                },
                per_unit: false,
            });
        }
        if let Some(unit_cost) = capture.name("unitcost") {
            cost = Some(Parsed::<Cost> {
                data: Cost {
                    amount: unit_cost.as_str().try_into()?,
                },
                per_unit: true,
            });
        } else if let Some(tot_cost) = capture.name("totcost") {
            cost = Some(Parsed::<Cost> {
                data: Cost {
                    amount: tot_cost.as_str().try_into()?,
                },
                per_unit: false,
            });
        }
    }
    if price.is_none() && cost.is_none() {
        Err(format!("unable to parse `{input}`"))
    } else {
        Ok((price, cost))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;
    use rust_decimal::{Decimal, prelude::FromPrimitive};

    #[test]
    fn test_tryfrom_transaction() -> Result<(), String> {
        let result = Transaction::try_from("2022-05-03 *")?;
        assert_eq!(result.postings.len(), 0);
        assert_eq!(result.date, date(2022, 5, 3));
        assert_eq!(result.flag, crate::core::types::TransactionFlag::OK);

        let result: Transaction = Transaction::try_from(
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

        let result: Transaction =
            Transaction::try_from("2022-05-03 *\n    Assets:Cash 5   CHF ; foobar\n    ")?;
        assert_eq!(result.postings.len(), 1);
        assert_eq!(result.postings[0].account, "Assets:Cash");
        assert_eq!(result.postings[0].amount.number, 5.into());
        assert_eq!(result.postings[0].amount.currency, "CHF");
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

    #[test]
    fn test_parse_price_and_cost() -> Result<(), String> {
        let success = vec![
            ("@3USD", Some((3.0, "USD", true)), None),
            ("@ 3.5 USD", Some((3.5, "USD", true)), None),
            ("  @@ 50   USD ", Some((50.0, "USD", false)), None),
            (
                " @ 1 USD {{ 6.3CHF}}",
                Some((1.0, "USD", true)),
                Some((6.3, "CHF", false)),
            ),
            (
                " @ 1 USD {6 CHF}",
                Some((1.0, "USD", true)),
                Some((6.0, "CHF", true)),
            ),
            (" {6 CHF}", None, Some((6.0, "CHF", true))),
            (" ", None, None),
            (
                " @@ 3USD {{60CHF}}",
                Some((3.0, "USD", false)),
                Some((60.0, "CHF", false)),
            ),
        ];
        let errors = vec![
            "5 USD",
            "{7 CHF",
            "{7 CHF }}",
            "{{7 CHF}",
            "@ CHF",
            "9 USD }",
            "@ 3 USD {5",
            "@ 5 CHF @@ 50 CHF",
            "@@ 50 CHF @ 3 usd",
            "{5 USD} {{ 20 CHF}}",
            "@ 5 USD {30 USD} {{3 chf}}",
            "{3 CHF  } @@ 5 USD",
        ];
        for (inp, expected_price, expected_cost) in success {
            let (price, cost) = parse_price_and_cost(inp)?;
            assert_eq!(
                price.is_some(),
                expected_price.is_some(),
                "failed to parse {}",
                inp
            );
            assert_eq!(
                cost.is_some(),
                expected_cost.is_some(),
                "failed to parse {}",
                inp
            );
            if let Some(res) = price {
                let expected = expected_price.unwrap();
                assert_eq!(
                    res.data.amount.number,
                    Decimal::from_f64(expected.0).unwrap(),
                    "failed to parse {}. Got {:?}",
                    inp,
                    res
                );
                assert_eq!(
                    res.data.amount.currency, expected.1,
                    "failed to parse {}. Got {:?}",
                    inp, res
                );
                assert_eq!(res.per_unit, expected.2, "failed to parse {}", inp);
            }
            if let Some(res) = cost {
                let expected = expected_cost.unwrap();
                assert_eq!(
                    res.data.amount.number,
                    Decimal::from_f64(expected.0).unwrap(),
                    "failed to parse {}. Got {:?}",
                    inp,
                    res
                );
                assert_eq!(
                    res.data.amount.currency, expected.1,
                    "failed to parse {}. Got {:?}",
                    inp, res
                );
                assert_eq!(res.per_unit, expected.2, "failed to parse {}", inp);
            }
        }
        for inp in errors {
            assert!(parse_price_and_cost(inp).is_err());
        }
        Ok(())
    }
}
