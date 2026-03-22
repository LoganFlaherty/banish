use banish::banish;

fn main() {
    banish! {
        @foo
            step? { return; } !? { return; }
    }
}
