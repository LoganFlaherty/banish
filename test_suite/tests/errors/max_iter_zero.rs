use banish::banish;

fn main() {
    banish! {
        #[max_iter = 0]
        @foo
            done? { return; }
    }
}
