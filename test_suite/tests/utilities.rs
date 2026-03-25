use banish::{BanishDispatch, banish};

/// Verify banish can be used inside a regular function and return a value
/// through the enclosing function's return statement.
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

/// Redirect fires on the (N+1)th entry, not the Nth.
/// Redirect target receives control and can itself transition further.
/// Verifies the redirect integrates correctly with the state scheduler.
#[test]
fn max_entry_can_state_transition() {
    let mut stage: u32 = 0;
    stage = banish! {
        #[max_entry = 2 => @middle]
        @start
            work? { stage += 1; }
            next? { => @start; }

        #[isolate]
        @middle
            advance? { stage += 10; => @end; }

        @end
            finish? { return stage; }
    };

    // @start runs twice (stage = 2), then @middle adds 10 (stage = 12)
    assert_eq!(stage, 12);
}

/// `break` exits the state loop immediately, skipping any remaining rules and
/// passes. The scheduler then advances to the next state normally.
#[test]
fn break_exits_state_early() {
    let mut log: Vec<&str> = Vec::new();

    log = banish! {
        @process
            step_a? { log.push("a"); }
            bail ? log.len() >= 1 { break; }
            step_b? { log.push("b"); } // Should never run

        @finish
            done? { return log; }
    };

    // step_b must never appear: bail fired before step_b's pass could run
    assert_eq!(log, vec!["a"]);
}

/// `continue` restarts rule evaluation from the top without advancing the
/// scheduler, equivalent to a rule firing and setting __interaction = true.
#[test]
fn continue_restarts_evaluation() {
    let mut x: usize = 0;

    x = banish! {
        @count
            inc ? x < 3 {
                x += 1;
                continue;
            }
            
            early_finish ? x < 3 { return x; }

            finish ? x == 3 { return x; }
    };

    // inc fired 3 times, each time restarting from the top via continue
    assert_eq!(x, 3);
}

/// A guarded transition fires when its condition is true, jumping to the target
/// state immediately. Statements after it in the rule body do not run.
#[test]
fn guarded_transition_fires_when_true() {
    let mut x: u32 = 0;
 
    x = banish! {
        @a
            go? {
                x = 1;
                => @b if x == 1;
                x = 2; // must not run
            }
            done? { return x; }
 
        @b
            finish? { return x; }
    };
 
    assert_eq!(x, 1);
}
 
/// A guarded transition does nothing when its condition is false. Execution
/// continues with the remaining statements in the rule body.
#[test]
fn guarded_transition_skips_when_false() {
    let mut x: u32 = 0;
 
    x = banish! {
        @a
            go? {
                => @b if x != 0; // Shouldn't be true
                x = 42;
                return x;
            }
 
        @b
            finish? { return 99; }
    };
 
    assert_eq!(x, 42);
}

/// Verify dispatch can convert HTTP to http
#[test]
fn dipatch_http() {
    #[derive(BanishDispatch)]
    enum DispatchEnum {HTTP}

    let dispatch = DispatchEnum::HTTP;

    let result = banish! {
        #![dispatch(dispatch)]

        @first
            wrong? { return false; }
        
        @http
            dispatched? { return true; }
    };

    assert_eq!(result, true)
}