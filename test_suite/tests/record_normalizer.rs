use banish::banish;

// Reproduces the Record Normalizer example from the reference.
// Verifies that the normalize state converges correctly across multiple passes,
// and that finalize sorts and deduplicates the result.
#[banish::machine]
#[tokio::test]
async fn record_normalizer() {
    let records = banish! {
        let mut records: Vec<String> = Vec::new();
        let mut load_error: Option<String> = None;
 
        @fetch
            load? {
                match tokio::fs::read_to_string("../test_suite/mock_data/records.txt").await {
                    Ok(content) => {
                        records = content.lines().map(str::to_string).collect();
                    }
                    Err(e) => {
                        load_error = Some(e.to_string());
                    }
                }
            }

            // Transition is a standalone statement here because transitions
            // cannot appear inside nested blocks such as match arms.
            bail ? load_error.is_some() { => @error; }
 
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
                let output = records.join("\n");
                tokio::fs::write("../test_suite/mock_data/records_clean.txt", output).await.expect("Write failed");
                println!("Wrote {} records to records_clean.txt", records.len());
                return records;
            }
 
        #[isolate]
        @error
            handle? {
                eprintln!("Failed to load records: {}", load_error.unwrap());
                return records;
            }
    };

    assert_eq!(records, vec!["alice", "bob", "charlie", "diana", "eve", "frank"]);
}