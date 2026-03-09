use banish::banish;

// Verify the scheduler skips an isolated state in the middle of the declaration
// order and advances directly from @first to @last.
#[test]
fn test_isolated_state_is_skipped_by_scheduler() {
    let visited_isolated: bool = banish! {
        @first
            setup? {
                // intentionally empty — just needs to converge and advance
            }

        #[isolate]
        @middle
            flag? {
                return true;
            }

        @last
            done? {
                return false;
            }
    };

    assert!(!visited_isolated, "Scheduler entered isolated state @middle");
}

// Verify the scheduler skips an isolated state at the end of the declaration
// order, treating the last non-isolated state as the terminal state.
#[test]
fn test_isolated_state_at_end_is_skipped_by_scheduler() {
    let visited_isolated: bool = banish! {
        @first
            done? {
                return false;
            }

        #[isolate]
        @cleanup
            flag? {
                return true;
            }
    };

    assert!(!visited_isolated, "Scheduler entered isolated state @cleanup");
}

// Verify that an isolated state is reachable via explicit transition and
// executes correctly, then redirects to the intended continuation state.
#[test]
fn test_isolated_state_reachable_via_transition() {
    let result: &str = banish! {
        @start
            go? {
                => @handler;
            }

        #[isolate, max_iter = 1 => @finish]
        @handler
            work? {
                println!("Isolated handler running");
            }

        @finish
            done? {
                return "ok";
            }
    };

    assert_eq!(result, "ok");
}