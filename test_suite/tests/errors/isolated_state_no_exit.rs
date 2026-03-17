use banish::banish;

fn main() {
    let mut x = 0;
    banish! {
        @entry
            go? { => @handler; }
            done? { return; }

        #[isolate]
        @handler
            handle? { x += 1; }
    }
}
