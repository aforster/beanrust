use beanrust::parser;
use std::{env, path};

#[test]
fn parse_file() -> Result<(), String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let ledger_path: path::PathBuf = [&manifest_dir, "tests/test_ledger.beancount"]
        .iter()
        .collect();
    assert!(ledger_path.exists(), "path: {:?}", ledger_path.to_str());
    let result = parser::parse_entries_from_file(&ledger_path).map_err(|e| e.to_string())?;
    assert!(!result.is_empty());
    assert_eq!(result.open.len(), 8);
    println!("{}", result.unhandled_entries.join("\n--\n"));
    assert_eq!(result.unhandled_entries.len(), 15);

    Ok(())
}
