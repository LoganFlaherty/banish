use banish::banish;

// Reproduces the Record Normalizer example from the reference.
// Verifies that the normalize state converges correctly across multiple passes,
// and that finalize sorts and deduplicates the result.
#[test]
fn record_normalizer() {
    let mut records: Vec<String> = vec![
        "  Alice  ".into(),
        "bob".into(),
        "  ALICE".into(),
        "".into(),
        "Charlie".into(),
        "bob".into(),
    ];

    records = banish! {
        @normalize
            trim ? records.iter().any(|r| r != r.trim()) {
                records = records.into_iter().map(|r| r.trim().to_string()).collect();
            }

            lowercase ? records.iter().any(|r| r != &r.to_lowercase()) {
                records = records.into_iter().map(|r| r.to_lowercase()).collect();
            }

            remove_empty ? records.iter().any(|r| r.is_empty()) {
                records.retain(|r| !r.is_empty());
            }

        @finalize
            dedup? {
                records.sort();
                records.dedup();
                return records;
            }
    };

    assert_eq!(records, vec!["alice", "bob", "charlie"]);
}