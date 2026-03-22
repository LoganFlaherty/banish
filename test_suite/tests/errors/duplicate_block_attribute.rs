use banish::banish;

fn main() {
    banish! {
        #![async, async]

        @foo
            done? { return; }
    }
}
