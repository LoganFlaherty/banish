use banish::banish;
use serde::Deserialize;

// Verify that #![async] expands to an async block, that .await is valid inside
// rule bodies, and that a return value is correctly propagated through .await.
#[tokio::test]
async fn async_block_returns_value() {
    let mut x: u32 = 0;

    let result: u32 = banish! {
        #![async]

        @count
            inc ? x < 3 { x += 1; }

            finish ? x == 3 { return x; }
    }.await;

    assert_eq!(result, 3);
}

// Verify that async works correctly across explicit state transitions, and that
// a value accumulated across states is returned correctly through .await.
#[tokio::test]
async fn async_multi_state_transition() {
    let mut log: Vec<&str> = Vec::new();

    let result: Vec<&str> = banish! {
        #![async]

        @first
            step? {
                log.push("first");
                => @second;
            }

        @second
            step? {
                log.push("second");
                => @third;
            }

        @third
            step? {
                log.push("third");
                return log;
            }
    }.await;

    assert_eq!(result, vec!["first", "second", "third"]);
}

// Reproduces the Async HTTP Fetch example from the reference.
// Fetches Charizard's data from the PokeAPI and verifies the returned name
// and that exactly 5 moves are returned.
// Requires a network connection.
#[tokio::test]
#[ignore]
async fn async_http_fetch_pokemon() {
    #[derive(Deserialize)]
    struct Pokemon {
        name: String,
        base_experience: u32,
        height: u32,
        weight: u32,
    }

    #[derive(Deserialize)]
    struct Move {
        name: String,
    }

    #[derive(Deserialize)]
    struct MoveEntry {
        #[serde(rename = "move")]
        move_data: Move,
    }

    #[derive(Deserialize)]
    struct PokemonMoves {
        moves: Vec<MoveEntry>,
    }

    let mut pokemon: Option<Pokemon> = None;

    let (pokedata, moves) = banish! {
        #![async]

        @fetch_pokemon
            load_pokemon? {
                let response = reqwest::get("https://pokeapi.co/api/v2/pokemon/charizard")
                    .await
                    .expect("request failed");
                pokemon = Some(
                    response.json::<Pokemon>().await.expect("failed to parse pokemon")
                );
            }

            load_pokemon_moves? {
                let response = reqwest::get("https://pokeapi.co/api/v2/pokemon/charizard")
                    .await
                    .expect("request failed");
                let data = response.json::<PokemonMoves>().await.expect("failed to parse moves");
                let moves: Vec<String> = data.moves
                    .iter()
                    .take(5)
                    .map(|m| m.move_data.name.clone())
                    .collect();
                return (pokemon, moves);
            }
    }.await;

    let p = pokedata.expect("pokemon should be present");
    assert_eq!(p.name, "charizard");
    assert!(p.base_experience > 0);
    assert!(p.height > 0);
    assert!(p.weight > 0);
    assert_eq!(moves.len(), 5);
}