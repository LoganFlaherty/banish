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

// Verify that an isolated state with max_iter correctly exhausts and
// transitions to the redirect target rather than looping forever.
#[test]
fn test_hello_world_with_max_iter_transition_completes() {
    banish! {
        #[isolate, trace, max_iter = 2 => @end]
        @hello
            print ? true {
                println!("Hello, world!");
            }

        @end
            print? {
                println!("Goodbye, world!");
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