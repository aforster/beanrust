use crate::core::types::*;

pub fn print_posting(posting: &Posting) -> String {
    let mut out = format!("    {} {}", posting.account, posting.amount);
    if let Some(price) = &posting.price {
        out.push_str(&format!(" @ {} ", price.amount));
    }
    if let Some(cost) = &posting.cost {
        match cost {
            CostType::Known(c) => {
                out.push_str(&format!(" {{ {} }} ", c.amount));
            }
            CostType::Automatic => {
                out.push_str(" { } ");
            }
        }
    }
    out.trim_end().to_string()
}

pub fn print_transaction(tx: &Transaction) -> String {
    let mut out = format!(
        "{} {}",
        tx.date,
        match tx.flag {
            TransactionFlag::OK => "*",
            TransactionFlag::Error => "!",
        }
    );
    if let Some(payee) = &tx.payee {
        out.push_str(&format!(" \"{}\"", payee));
    }
    if let Some(narration) = &tx.narration {
        out.push_str(&format!(" \"{}\"", narration));
    } else if tx.payee.is_some() {
        out.push_str(" \"\"");
    }
    for p in &tx.postings {
        out.push('\n');
        out.push_str(&print_posting(&p));
    }
    out
}

#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn test_print_posting() {
        let acc = "Assets:Cash".to_string();
        let am = Amount::new(100.into(), "USD".to_string());
        let posting = Posting {
            account: acc.clone(),
            amount: am.clone(),
            price: None,
            cost: None,
        };
        assert_eq!(
            print_posting(&posting)
                .split(' ')
                .filter(|e| !e.is_empty())
                .collect::<Vec<&str>>(),
            ["Assets:Cash", "100", "USD"]
        );

        let posting = Posting {
            account: acc.clone(),
            amount: am.clone(),
            price: Some(Price {
                amount: "50 CHF".try_into().unwrap(),
            }),
            cost: None,
        };
        assert_eq!(
            print_posting(&posting)
                .split(' ')
                .filter(|e| !e.is_empty())
                .collect::<Vec<&str>>(),
            ["Assets:Cash", "100", "USD", "@", "50", "CHF"]
        );

        let posting = Posting {
            account: acc.clone(),
            amount: am.clone(),
            price: None,
            cost: Some(CostType::Known(Cost {
                amount: "50 CHF".try_into().unwrap(),
            })),
        };
        assert_eq!(
            print_posting(&posting)
                .split(' ')
                .filter(|e| !e.is_empty())
                .collect::<Vec<&str>>(),
            ["Assets:Cash", "100", "USD", "{", "50", "CHF", "}"]
        );

        let posting = Posting {
            account: acc.clone(),
            amount: am.clone(),
            price: Some(Price {
                amount: "75 CHF".try_into().unwrap(),
            }),
            cost: Some(CostType::Known(Cost {
                amount: "50 CHF".try_into().unwrap(),
            })),
        };
        assert_eq!(
            print_posting(&posting)
                .split(' ')
                .filter(|e| !e.is_empty())
                .collect::<Vec<&str>>(),
            [
                "Assets:Cash",
                "100",
                "USD",
                "@",
                "75",
                "CHF",
                "{",
                "50",
                "CHF",
                "}"
            ]
        );
    }

    #[test]
    fn test_print_transaction() {
        let t = Transaction {
            date: date(2022, 5, 3),
            flag: TransactionFlag::OK,
            payee: None,
            narration: None,
            postings: vec![],
        };
        assert_eq!(print_transaction(&t), "2022-05-03 *");
        let t = Transaction {
            date: date(2022, 5, 3),
            flag: TransactionFlag::OK,
            payee: None,
            narration: Some("foo".to_string()),
            postings: vec![],
        };
        assert_eq!(print_transaction(&t), "2022-05-03 * \"foo\"");
        let t = Transaction {
            date: date(2022, 5, 3),
            flag: TransactionFlag::OK,
            payee: Some("foo".to_string()),
            narration: None,
            postings: vec![],
        };
        assert_eq!(print_transaction(&t), "2022-05-03 * \"foo\" \"\"");
        let t = Transaction {
            date: date(2022, 5, 3),
            flag: TransactionFlag::OK,
            payee: Some("bar".to_string()),
            narration: Some("foo".to_string()),
            postings: vec![],
        };
        assert_eq!(print_transaction(&t), "2022-05-03 * \"bar\" \"foo\"");

        let t = Transaction {
            date: date(2022, 5, 3),
            flag: TransactionFlag::Error,
            payee: None,
            narration: Some("foo".to_string()),
            postings: vec![],
        };
        assert_eq!(print_transaction(&t), "2022-05-03 ! \"foo\"");

        let t = Transaction {
            date: date(2022, 5, 3),
            flag: TransactionFlag::OK,
            payee: None,
            narration: None,
            postings: vec![
                Posting {
                    account: "Assets:Cash".to_string(),
                    amount: "5 CHF".try_into().unwrap(),
                    price: None,
                    cost: None,
                },
                Posting {
                    account: "Assets:Cash2".to_string(),
                    amount: "5 USD".try_into().unwrap(),
                    price: None,
                    cost: None,
                },
            ],
        };
        assert_eq!(
            print_transaction(&t),
            "2022-05-03 *\n    Assets:Cash 5 CHF\n    Assets:Cash2 5 USD"
        );
    }
}
