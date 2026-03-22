use banish::banish;

// Verify the traffic light cycle compiles and terminates.
// max_entry = 2 on @red means the machine exits after two full red-green-yellow cycles.
#[test]
fn test_traffic_lights_completes() {
    banish! {
        let mut ticks: i32 = 0;

        #[max_entry = 2]
        @red
            announce ? {
                ticks = 0;
                println!("Red light");
            }
            timer ? ticks < 3 {
                ticks += 1;
            }

        @green
            announce ? {
                println!("Green light");
            }
            timer ? ticks < 6 {
                ticks += 1;
            }

        @yellow
            announce ? {
                println!("Yellow light");
            }
            timer ? ticks < 10 {
                ticks += 1;
            } !? { => @red; }
    }
}

// Verify the machine loops through all three states exactly twice before
// returning, and that loop_count reflects that correctly.
#[test]
fn test_traffic_lights_loop_count() {
    let mut ticks: i32 = 0;
    let mut loop_count: i32 = 0;
    let loop_count = banish! {
        @red
            announce ? {
                ticks = 0;
                println!("Red light");
            }
            timer ? ticks < 3 {
                ticks += 1;
            }

        @green
            announce ? {
                println!("Green light");
            }
            timer ? ticks < 6 {
                ticks += 1;
            }

        @yellow
            announce ? {
                println!("Yellow light");
            }
            timer ? ticks < 10 {
                ticks += 1;
            } !? {
                loop_count += 1;
                => @red;
            }

            end ? loop_count == 1 { return loop_count; }
    };

    assert_eq!(loop_count, 1);
}