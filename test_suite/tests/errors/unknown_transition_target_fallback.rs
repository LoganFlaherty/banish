use banish::banish;

fn main() {
    banish! {
        @foo
            step ? false { } !? { => @nowhere; }
    }
}
