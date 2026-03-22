use banish::banish;

#[banish::machine]
fn main() {
    // Set up to test any ideas that come to mind without writing a full test.
    banish! {
        @last
            done? {
                if true { break; };
            }
            doneret? { return; }
    }
}