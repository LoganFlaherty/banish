use banish::banish;

fn main() {
    let x = true;
    banish! {
        @entry
            go? { => @handler; }

        // A guarded transition is conditional and cannot guarantee exit,
        // so it must not satisfy the isolated state exit requirement.
        #[isolate]
        @handler
            step? { => @entry if x; }
    }
}