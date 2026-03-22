use banish::banish;

fn main() {
    banish! {
        #[max_entry = 1 => @nowhere]
        @foo
            step? { return; }
    }
}
