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