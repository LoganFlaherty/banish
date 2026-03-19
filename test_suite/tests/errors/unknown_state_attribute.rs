use banish::banish;

fn main() {
    banish! {
        #[unknown_attr]
        @foo
            done? { return; }
    }
}
