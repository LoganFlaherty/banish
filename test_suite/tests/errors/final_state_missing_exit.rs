use banish::banish;

fn main() {
    let mut x = 0;
    banish! {
        @foo
            step ? x > 0 { x -= 1; }
    }
}
