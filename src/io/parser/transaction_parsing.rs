use crate::core::types::*;

impl TryFrom<&str> for Transaction {
    type Error = String;
    fn try_from(statement: &str) -> Result<Self, Self::Error> {
        Err("not implemented".to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;
    #[test]
    fn test_tryfrom_transaction() -> Result<(), String> {
        let result =
            Transaction::try_from("2022-05-03 *\n    Assets:Cash 5 CHF\n    Assets:Cash2 5 USD")?;
        assert_eq!(result.postings.len(), 2);
        assert_eq!(result.date, date(2022, 5, 3));
        assert_eq!(result.flag, crate::core::types::TransactionFlag::OK);

        Ok(())
    }
}
