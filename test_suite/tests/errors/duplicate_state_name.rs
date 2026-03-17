use banish::banish;

fn main() {
    banish! {
        @foo
            done? { return; }

        @foo
            done? { return; }
    }
}
