use banish::banish;

/// A failed condition still counts. The body must not run.
fn consider_false(checks: &mut u32) -> bool {
    *checks += 1;
    false
}

/// `#[max_entry = 2]` allows two considerations, then skips only that rule.
/// `other` keeps running, so the result is `(a, b) == (2, 3)`.
#[test]
fn rule_max_entry_skips_only_that_rule() {
    let mut a: u32 = 0;
    let mut b: u32 = 0;

    let result: (u32, u32) = banish! {
        @work
            #[max_entry = 2]
            limited ? a < 10 { a += 1; }
            other ? b < 3 { b += 1; }
            done ? b >= 3 { return (a, b); }
    };

    assert_eq!(result, (2, 3));
}

/// The counter lives for the whole `banish!` block. Re-entering the state
/// does not reset it, so the capped rule does not run a second time.
#[test]
fn rule_max_entry_persists_across_reentry() {
    let mut hits: u32 = 0;
    let mut visits: u32 = 0;

    let hits: u32 = banish! {
        @start
            #[max_entry = 1]
            once? { hits += 1; }
            leave ? visits < 1 {
                visits += 1;
                => @again;
            }
            finish? { return hits; }

        #[isolate]
        @again
            back? { => @start; }
    };

    assert_eq!(hits, 1);
}

/// `#[...]` before a rule name is a rule attribute. `#[...]` before `@name`
/// is still a state attribute.
#[test]
fn rule_attribute_then_state_attribute_parses() {
    let mut ran_limited = false;
    let mut ran_isolated = false;

    let (ran_limited, ran_isolated): (bool, bool) = banish! {
        @first
            #[max_entry = 1]
            limited? {
                ran_limited = true;
                => @other;
            }
            done? { return (ran_limited, ran_isolated); }

        #[isolate]
        @other
            go? {
                ran_isolated = true;
                return (ran_limited, ran_isolated);
            }
    };

    assert!(ran_limited);
    assert!(ran_isolated);
}

/// Each time the rule is reached counts, including when its condition is false.
/// After the cap, the condition is not evaluated again.
#[test]
fn rule_max_entry_counts_each_consideration() {
    let mut checks: u32 = 0;
    let mut passes: u32 = 0;

    let checks: u32 = banish! {
        @work
            #[max_entry = 1]
            limited ? consider_false(&mut checks) { return 99; }
            tick ? passes < 3 { passes += 1; }
            done ? passes >= 3 { return checks; }
    };

    assert_eq!(checks, 1);
}
