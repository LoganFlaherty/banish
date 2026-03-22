use banish::banish;

fn main() {
    banish! {
        #[trace, trace]
        @foo
            done? { return; }
    }
}
