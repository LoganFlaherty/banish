use banish::banish;

fn main() {
    banish! {
        #[max_entry = 0]
        @foo
            done? { return; }
    }
}
