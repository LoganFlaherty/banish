use banish::banish;

fn main() {
    banish! {
        @foo
            go? { => @nowhere; }
    }
}
