use banish::banish;

// Verify that a self-transitioning state correctly implements nested loop
// semantics: x counts to 10, resets, y increments, repeats until y == 10.
// Total increments of x should be 100 (10 * 10).
#[test]
fn test_double_for_loop_completes() {
    let mut x = 0;
    let mut y = 0;
    let mut cnt = 0;
    let (x, y, cnt) = banish! {
        @for_loops
            for_x ? x != 10 {
                x += 1;
                cnt += 1;
            } !? {
                x = 0;
                y += 1;
                if y == 10 { return (x, y, cnt); }
                => @for_loops;
            }
    };

    assert_eq!(x, 0);
    assert_eq!(y, 10);
    assert_eq!(cnt, 100);
}