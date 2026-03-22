use banish::banish;

fn main() {
    let x = true;
    banish! {
        // A guarded transition is conditional and cannot guarantee exit,
        // so it must not satisfy the final state exit requirement.
        @foo
            step? { => @foo if x; }
    }
}