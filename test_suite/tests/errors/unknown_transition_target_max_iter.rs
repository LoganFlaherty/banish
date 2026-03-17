use banish::banish;

fn main() {
    banish! {
        #[max_iter = 3 => @nowhere]
        @foo
            step? { return; }
    }
}
