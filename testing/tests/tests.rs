#[cfg(test)]
mod tests {
    use banish::banish;
    use rand::prelude::*;

    // Ensure it compiles and runs without panicking
    #[test]
    fn test_hello_world_completes() {
        banish! {
            @hello
                print? {
                    println!("Hello, world!");
                    return;
                }
        }
    }

    // Ensure it compiles and runs without panicking
    #[test]
    fn test_traffic_lights_completes() {
        let mut ticks: i32 = 0;
        let mut loop_count: i32 = 0;
        banish! {
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

                end ? loop_count == 1 { return; }
        }
    }

    // Ensure it reaches loop_count == 1, meaning it looped twice through all states
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

    // Make sure it compiles with external library use
    #[test]
    fn test_dragon_fight_completes() {
        let mut rng = rand::rng();
        let mut player_hp = 1;
        let mut dragon_hp = 50;
        
        println!("BATTLE START");

        let result: &str = banish! {
            @player_turn
                // Conditionless Rule: Player attacks dragon
                attack ? {
                    let damage = rng.random_range(5..15); // Using external lib!
                    dragon_hp -= damage;
                    println!("You hit the dragon for {} dmg! (Dragon HP: {})", damage, dragon_hp);
                }

                check_win ? dragon_hp <= 0 {
                    return "Victory!"; // Early exit with value
                }

                end_turn ? {
                    => @dragon_turn; // Explicit transition else player just keeps attacking forever
                }

            @dragon_turn
                attack ? {
                    let damage = rng.random_range(2..20);
                    player_hp -= damage;
                    println!("Dragon breathes fire for {} dmg! (Player HP: {})", damage, player_hp);
                }

                halfway ? player_hp <= 10 && dragon_hp <= 25 {
                    println!("\nThe battle is getting intense!\n");
                } !? { println!("\nThe dragon is getting weak!\n"); } // Else clause

                check_loss ? player_hp <= 0 {
                    return "Defeat...";
                }

                end_turn ? {
                    => @player_turn;
                }
        };

        assert_eq!(result, "Defeat...");
    }

    // Test banish returning a value as a function return
    fn find_index(buffer: &[String], target: &str) -> Option<usize> {
        let mut idx = 0;
        banish! {
            @search
                not_found ? idx >= buffer.len() {
                    return None;
                }
                found ? buffer[idx] != target {
                    idx += 1;
                } !? { return Some(idx); }
        }
    }

    #[test]
    fn test_find_index_found_some() {
        let buffer = ["No".to_string(), "hey".to_string()];
        assert_eq!(find_index(&buffer, "hey"), Some(1));
    }

    // Just a general logic test
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
}