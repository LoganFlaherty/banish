use banish::banish;

// Verify the scheduler skips an isolated state in the middle of the declaration
// order and advances directly from @first to @last.
#[test]
fn test_isolated_state_is_skipped_by_scheduler() {
    let visited_isolated: bool = banish! {
        @first
            setup? {
                // intentionally empty. Just needs to converge and advance
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

// Verify that an isolated state are skipped as entry points.
#[test]
fn test_isolated_state_as_first_state() {
    let result = banish! {
        #[isolate]
        @first
            ret? {
                return true;
            }

        @second
            ret? {
                return false;
            }
    };

    assert!(!result, "Isolated state was entry point.");
}