use banish::banish;

fn main() {
    let x = true;
    banish! {
        @foo
            step? { => @foo if x }
    }
}