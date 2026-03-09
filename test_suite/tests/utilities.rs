use banish::banish;

// Verify banish can be used inside a regular function and return a value
// through the enclosing function's return statement.
fn find_index(buffer: &[String], target: &str) -> Option<usize> {
    let mut idx = 0;
    banish! {
        @search
            not_found ? idx >= buffer.len() {
                return None;
            }
            found ? buffer[idx] != target {
                idx += 1;
            } !? { return Some(idx); }
    }
}

#[test]
fn test_find_index_found_some() {
    let buffer = ["No".to_string(), "hey".to_string()];
    assert_eq!(find_index(&buffer, "hey"), Some(1));
}

#[test]
fn test_find_index_not_found_none() {
    let buffer = ["No".to_string(), "hey".to_string()];
    assert_eq!(find_index(&buffer, "missing"), None);
}