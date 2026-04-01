use banish::banish;

/// `let Some(x) = queue.pop()` fires while there are items, binding the value
/// into the rule body. Once the queue is empty the pattern no longer matches
/// and the state reaches its fixed point.
#[test]
fn let_pattern_drains_queue() {
    let mut queue: Vec<i32> = vec![1, 2, 3];
    let mut sum: i32 = 0;

    (sum, queue) = banish! {
        @drain
            pop ? let Some(val) = queue.pop() {
                sum += val;
            } !? { return (sum, queue); }
    };

    assert_eq!(sum, 6);
    assert!(queue.is_empty());
}

/// `let Ok(n) = result` fires on success and binds the inner value.
/// The fallback `!?` branch handles the error case.
#[test]
fn let_pattern_ok_variant() {
    let inputs: Vec<Result<i32, &str>> = vec![Ok(10), Err("bad"), Ok(5)];
    let mut iter: std::vec::IntoIter<Result<i32, &str>> = inputs.into_iter();
    let mut successes: Vec<i32> = Vec::new();
    let mut errors: usize = 0;

    (successes, errors) = banish! {
        @process
            next ? let Some(item) = iter.next() {
                if let Ok(n) = item {
                    successes.push(n);
                } else {
                    errors += 1;
                }
            } !? { return (successes, errors); }
    };

    assert_eq!(successes, vec![10, 5]);
    assert_eq!(errors, 1);
}
