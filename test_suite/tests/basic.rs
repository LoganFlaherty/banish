use banish::banish;

// Verify a minimal single-state machine compiles and runs.
#[test]
fn test_hello_world_completes() {
    banish! {
        @hello
            print? {
                println!("Hello, world!");
                return;
            }
    }
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