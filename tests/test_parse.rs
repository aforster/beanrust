use beanrust::parser;
use std::{env, path};

#[test]
fn parse_file() -> Result<(), String> {
    /* let exe_env = env!("CARGO_BIN_EXE_TEST_PARSE");
    let ledger_path: path::PathBuf = [
        path::Path::new(&exe_env).parent().unwrap(),
        path::Path::new("test_ledger.beancount"),
    ]
    .iter()
    .collect();
    assert!(ledger_path.exists(), "path: {:?}", ledger_path.to_str());
    let result = parser::parse_entries_from_file(&ledger_path)?;*/
    Ok(())
}
