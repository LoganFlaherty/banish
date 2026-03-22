use banish::banish;
use rand::prelude::*;

// Verify a multi-state machine that uses an external crate (rand) and max_entry compiles and
// produces a determinate result. player_hp = 1 guarantees the dragon wins on
// the first attack, so the result is always "Defeat...".
#[test]
fn test_dragon_fight_completes() {
    println!("BATTLE START");

    let result: &str = banish! {
        let mut rng = rand::rng();
        let mut player_hp = 1;
        let mut dragon_hp = 50;

        #[max_iter = 1 => @dragon_turn]
        @player_turn
            attack? {
                let damage = rng.random_range(5..15);
                dragon_hp -= damage;
                println!("You hit the dragon for {} dmg! (Dragon HP: {})", damage, dragon_hp);
            }

            check_win ? dragon_hp <= 0 { return "Victory!"; }

        #[max_iter = 1 => @player_turn]
        @dragon_turn
            attack? {
                let damage = rng.random_range(2..20);
                player_hp -= damage;
                println!("Dragon breathes fire for {} dmg! (Player HP: {})", damage, player_hp);
            }

            half_health ? player_hp <= 10 && dragon_hp <= 25 {
                println!("\nThe battle is getting intense!\n");
            } !? {
                println!("\nThe dragon is getting weak!\n");
            }

            check_loss ? player_hp <= 0 { return "Defeat..."; }
    };

    assert_eq!(result, "Defeat...");
}