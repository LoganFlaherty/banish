use banish::banish;

async fn main() {
    banish! {
        #![async]
        #![async]

        #[max_iter = 5]
        @foo
            done? { return; }
    }
}
