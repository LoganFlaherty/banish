use banish::banish;

fn main() {
    let mut x = 0;
    banish! {
        @entry
            go? { => @handler; }
            done? { return; }

        // max_entry redirect only fires on the (N+1)th entry.
        // Entries 1 through N have no exit, so this must be rejected.
        #[isolate, max_entry = 1 => @entry]
        @handler
            handle? { x += 1; }
    }
}
