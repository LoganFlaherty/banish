use banish::banish;

fn main() {
    banish! {
        #[trace]
        #[max_iter = 5]
        @foo
            done? { return; }
    }
}
