use banish::banish;

fn main() {
    banish! {
        @foo
            step ? x > 0
    }
}