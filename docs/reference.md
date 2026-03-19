# Banish Reference

This document is the full technical reference for Banish. For a quick introduction, see the [main README](https://github.com/LoganFlaherty/banish/blob/main/README.md). For questions or feedback, open a [Discussion](https://github.com/LoganFlaherty/banish/discussions).

---

## Table of Contents
1. [Execution Model](#execution-model)
2. [States](#states)
3. [Rules](#rules)
4. [Transitions](#transitions)
5. [Return Values](#return-values)
6. [State Attributes](#state-attributes)
7. [Block Attributes](#block-attributes)
8. [Examples](#examples)
9. [Error Reference](#error-reference)
10. [Known Limitations](#known-limitations)

---

## Execution Model

A `banish!` block expands to a labeled loop containing a `match` over an internal state index. The machine starts at the first non-isolated state in declaration order and advances through non-isolated states in declaration order. Isolated states are skipped by the scheduler and can only be entered via an explicit `=> @state` transition.

Within each state, rules are evaluated top to bottom on every pass. If any rule fires, `__interaction` is set to `true` and the state loops back to the top, re-evaluating all rules from the beginning. Once a full pass completes with no rules firing (the fixed point) the machine advances to the next non-isolated state.

```
  enter state
       │
       ▼
  evaluate rules top to bottom
       │
       ├─── any rule fired? ── yes ──► re-evaluate from top
       │
       no
       │
       ▼
  fixed point reached
       │
       ▼
  advance to next non-isolated state
```

All of this is generated at compile time. There is no runtime interpreter, allocator, or virtual machine.

---

## States

A state is declared with `@name` and contains one or more rules. States are evaluated in declaration order by the implicit scheduler.

```rust
banish! {
    @first
        // Rules
        ...

    @second
        // Rules
        ...
}
```

The machine always starts at the first non-isolated state in declaration order. The implicit scheduler advances to the next non-isolated state once the current state reaches its fixed point. States can be removed from implicit scheduling with the [`isolate`](#isolate) attribute.

**Naming:** State names follow standard Rust identifier rules. Duplicate state names are a compile error.

---

## Rules

### Conditional Rules

```
name ? condition { body }
```

A rule fires when `condition` evaluates to `true`. The condition is any Rust expression that evaluates to `bool`. After firing, the state re-evaluates from the top. If the condition is `false`, the rule is skipped silently.

```rust
@process
    clamp ? value > 100 { value = 100; }

    log ? ready { println!("{}", value); ready = false; }
```

### Conditionless Rules

```
name ? { body }
```

A conditionless rule fires exactly once per state entry, on the first pass only. It sets `__interaction = true` on that pass, causing the state to re-evaluate, but it will not fire again on subsequent passes within the same entry. Conditionless rules cannot have a fallback branch.

```rust
@setup
    init? {
        value = compute();
        println!("Initialized");
    }
    // Remaining rules run after init fires once
```

### Fallback Branches

```
name ? condition { body } !? { else_body }
```

A fallback branch runs when the rule's condition is `false`. Unlike the rule body, a fallback branch does **not** set `__interaction = true`. It does not trigger a re-evaluation pass on its own. If you need to loop after a fallback, use an explicit `=> @state` transition.

```rust
@check
    valid ? value > 0 {
        println!("Valid: {}", value);
    } !? {
        println!("Invalid, resetting");
        value = default;
        => @check; // Explicit re-entry needed here
    }
```

**Rule ordering matters.** Rules are evaluated top to bottom on every pass. A rule earlier in the list can change state that affects whether a later rule fires. Design rule order accordingly.

**Duplicate rule names** within a state are a compile error.

---

## Transitions

### Implicit (Scheduler)

Once a state reaches its fixed point, the machine automatically advances to the next non-isolated state in declaration order. This is the default behavior and requires no syntax.

### Explicit (`=> @state`)

```
=> @state;
```

An explicit transition immediately jumps to the named state, bypassing the implicit scheduler. It must appear as a standalone statement in a rule body or fallback branch. The remaining rules in the current pass are abandoned and the target state begins a fresh evaluation.

```rust
@yellow
    timer ? ticks < 10 {
        ticks += 1;
    } !? {
        ticks = 0;
        => @red;  // Jump back to @red directly
    }
```

Transition targets must refer to a declared state name. Unknown targets are a compile error at the `=> @state` callsite.

### Guarded Transitions (`=> @state if condition`)

```
=> @state if condition;
```

A guarded transition jumps to the named state only when `condition` evaluates to `true`. If the condition is `false`, the statement is a no-op and execution continues with the remaining statements in the rule body. Like an explicit transition, it must appear as a standalone statement.

```rust
@process
    step? {
        do_work();
        => @done if finished;
        log_progress(); // runs only when guard is false
    }

    timeout ? elapsed > limit { => @abort; }
```

This handles the most common case of needing a conditional jump without splitting the logic into a separate rule. The guard condition is any Rust expression that evaluates to `bool`.

**Guarded transitions do not satisfy the exit requirement** for isolated states or the final-state validator. Because the guard may never be true, a state relying solely on guarded transitions has no guaranteed exit path. A non-guarded `=> @state` or `return` is still required.

### Early Exit (`break` / `continue`)

Because rule bodies are plain Rust, the native loop control keywords work as-is against the generated fixed-point loop.

- `break;` exits the current state's loop immediately, skipping any remaining rules and passes, and lets the scheduler advance to the next state normally.
- `continue;` abandons the remainder of the current pass and restarts rule evaluation from the top, equivalent to a rule firing and setting `__interaction = true` manually.

```rust
@process
    step ? !done { do_work(); }

    bail ? error  { break; }   // exit state early, no transition needed
```

---

## Return Values

`return` exits the entire `banish!` block immediately, optionally with a value. The block evaluates to that value, so it can be assigned or returned from an enclosing function.
 
```rust
let result: &str = banish! {
    @grade
        pass ? score >= 60 {
            return "pass";
        } !? { return "fail"; }
};
```
 
The return type is inferred by the Rust compiler from the `return` expressions in the block. If the block returns nothing, it evaluates to `()`.
 
When `banish!` is the tail expression of a function, its value is the function's return value implicitly. `return` inside the block exits the generated closure, and the closure's value becomes the function's result.
 
```rust
fn classify(score: u32) -> &'static str {
    banish! {
        @grade
            pass ? score >= 60 {
                return "pass"; 
            } !? { return "fail"; }
    }
}
```

---

## State Attributes

Attributes are declared above a state and modify its runtime behavior. Multiple attributes are comma-separated.

```rust
#[isolate, max_iter = 10 => @fallback, trace]
@my_state
    // Rules
    ...
```

---

### `isolate`

Removes the state from implicit scheduling. An isolated state is never entered by the scheduler. It can only be reached via an explicit `=> @state` transition.

```rust
banish! {
    @main
        trigger ? condition {
            => @handler;
        } !? { return; }

    #[isolate, max_iter = 1 => @main]
    @handler
        handle? {
            println!("Handling");
        }
}
```

**Constraints:**
* An isolated state must have a defined exit: either a `return`, `=> @state` in its rules, or `max_iter = N => @state`. Isolated states with no exit are a compile error. `max_entry = N => @state` does not satisfy this requirement because it only fires on the (N+1)th entry and provides no exit for entries 1 through N. `=> @state if condition` does not satisfy this requirement because the guard may never be true.

---

### `max_iter = N`

Caps the fixed-point loop to `N` iterations. If the state has not converged after `N` iterations, the loop exits and the machine advances to the next state normally.

```rust
#[max_iter = 5]
@process
    step ? !done { do_work(); }
```

Useful as a safety net on states that could theoretically loop many times, or to limit processing per scheduler pass.

---

### `max_iter = N => @state`

Same as `max_iter = N`, but instead of advancing normally on exhaustion, the machine transitions to the named state.

```rust
#[max_iter = 3 => @timeout]
@retry
    attempt ? !succeeded { try_request(); }

#[isolate]
@timeout
    handle? { log_failure(); return; }
```

When a redirect is present, the state's `scheduler_advance` is never emitted. The only exit path is the redirect transition.

---

### `max_entry = N`

Limits the number of times a state can be entered. On the `(N+1)`th entry the state returns immediately without evaluating any rules.

```rust
#[max_entry = 2]
@red
    announce? { println!("Red light"); ticks = 0; }

    timer ? ticks < 3 { ticks += 1; }
```

In the traffic light example above, the machine exits after `@red` is entered a third time. The entry counter persists for the lifetime of the `banish!` block and is not reset between cycles.

**Note:** `max_entry = N => @state` on an isolated state does not remove the requirement for a rule-level exit. The redirect only fires on the (N+1)th entry. Entries 1 through N still need a `return`, `=> @state`, or `max_iter = N => @state` to exit normally.

---

### `trace`

Emits diagnostics via [`log::trace!`](https://docs.rs/log) on state entry and before each rule evaluation. Requires a `log`-compatible backend to capture output. Without one, diagnostics are silently discarded.

```rust
#[trace]
@compute
    step_a ? x > 0  { x -= 1; }

    step_b ? x == 0 { return; }
```

Output format:
```
[banish] entering state `compute`
[banish] rule `step_a`: condition = true
[banish] rule `step_b`: condition = false
```

[`env_logger`](https://docs.rs/env_logger) is the simplest backend. Add it to your `Cargo.toml`:

```toml
[dependencies]
env_logger = "0.11.9"
```

Initialize it at startup and run with `RUST_LOG=trace`:

```rust
fn main() {
    env_logger::init();
    ...
}
```

```bash
# bash / zsh
RUST_LOG=trace cargo run -q 2> trace.log

# PowerShell
$env:RUST_LOG="trace"; cargo run -q 2> trace.log
```

`log` itself is re-exported from `banish` and does not need to be added as a separate dependency.

---

## Block Attributes
 
Block attributes are declared at the top of a `banish!` block using inner attribute syntax and modify the behavior of the entire block. Multiple attributes are comma-separated.
 
```rust
banish! {
    #![async]
 
    @my_state
        ...
}
```
 
The `#![...]` line must appear before the first state declaration. Only one `#![...]` block is permitted per `banish!` invocation.
 
---
 
### `async`
 
Expands the `banish!` block to an `async move { ... }` expression instead of an immediately invoked closure. The result is a `Future` and must be `.await`ed by the caller.
 
```rust
let result = banish! {
    #![async]
 
    @fetch
        load? {
            let data = some_async_fn().await;
            return data;
        }
}.await;
```
 
**When to use it:** `async` is required any time a rule body contains `.await`. Without it, `.await` inside a rule body is a compile error because the generated closure is not async.
 
**What it changes:** Without `#![async]` the block expands to `(move || { ... })()`. With `#![async]` it expands to `async move { ... }`, which suspends at each `.await` point and must be driven by an async runtime such as `tokio`.
 
**Return values** work the same way as in a synchronous block. `return expr;` exits the block with a value; the resolved type of the `Future` is inferred from the `return` expressions.
 
```rust
#[tokio::main]
async fn main() {
    let status: &str = banish! {
        #![async]
 
        @check
            ping? {
                let ok = reqwest::get("https://example.com").await.is_ok();

                if ok { return "up"; }
                else { return "down"; }
            }
    }.await;
 
    println!("Status: {}", status);
}
```
 
**Combining with state attributes:** `#![async]` is independent of state-level attributes. Meaning they all work normally inside an async block.

---

## Examples

### Traffic Lights

A simple state machine that demonstrates implicit state advancement, conditionless rules, fallback transitions, and using the state attribute `max_entry`.

```rust
use banish::banish;

fn main() {
    let mut ticks: i32 = 0;
    banish! {
        #[max_entry = 2]
        @red
            announce? {
                ticks = 0;
                println!("\nRed light");
            }

            timer ? ticks < 3 { ticks += 1; }

        @green
            announce? { println!("Green light"); }

            timer ? ticks < 6 { ticks += 1; }

        @yellow
            announce? { println!("Yellow light"); }

            timer ? ticks < 10 {
                ticks += 1;
            } !? { => @red; }
    }
}
```

`@red` and `@green` each reach their fixed point once their timer rule stops firing. `@yellow`'s fallback branch transitions back to `@red` explicitly. Without this, fallback branches do not trigger re-evaluation. `max_entry = 2` ensures this machine only loops through all its states twice. Immediately returning on the third entry of `@red`.

---

### Dragon Fight

A turn-based battle that demonstrates early return with a value, external crate usage, multi-state transitions, fallback branches, and using the state attribute `max_iter` with the transition option. Requires `rand`.

```toml
[dependencies]
rand = "0.10.0"
```

```rust
use banish::banish;
use rand::prelude::*;

fn main() {
    let mut rng = rand::rng();
    let mut player_hp = 20;
    let mut dragon_hp = 50;

    println!("\n==== Battle Start ====");

    let result: &str = banish! {
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

    println!("GAME OVER: {}", result);
}
```

`max_iter = 1` on each state caps the fixed-point loop to a single iteration. After `attack?` fires and `check_win` or `check_loss` has been evaluated, if neither returns, the iteration limit is exhausted and the redirect transitions to the opposing state.

---

### Record Normalizer
 
A multi-pass normalization pipeline demonstrated with fixed-point looping. Each rule independently checks whether its transformation is still needed, making the state self-stabilizing without manual loop management.
 
```rust
use banish::banish;
 
fn main() {
    let mut records: Vec<String> = vec![
        "  Alice  ".into(),
        "bob".into(),
        "  ALICE".into(),
        "".into(),
        "Charlie".into(),
        "bob".into(),
    ];
 
    banish! {
        @normalize
            trim ? records.iter().any(|r| r != r.trim()) {
                records = records.into_iter().map(|r| r.trim().to_string()).collect();
            }
 
            lowercase ? records.iter().any(|r| r != &r.to_lowercase()) {
                records = records.into_iter().map(|r| r.to_lowercase()).collect();
            }
 
            remove_empty ? records.iter().any(|r| r.is_empty()) {
                records.retain(|r| !r.is_empty());
            }
 
        @finalize
            dedup? {
                records.sort();
                records.dedup();
                println!("{:?}", records); // ["alice", "bob", "charlie"]
                return;
            }
    }
}
```
 
`@normalize` re-evaluates until all three rules stop firing. Each pass that changes the data triggers another, converging when the records are fully trimmed, lowercased, and non-empty. `@finalize` sorts and deduplicates on a single conditionless pass, then returns.
 
---

### Async HTTP Fetch

An async workflow that demonstrates `#![async]`, `.await`, `#[trace]`, external crate usage, and returning a tuple value from an async block. Requires `tokio`, `reqwest`, `serde`, and `env_logger`.

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
env_logger = "0.11.9"
```

```rust
use banish::banish;
use serde::Deserialize;

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

#[tokio::main]
async fn main() {
    env_logger::init();
    let mut pokemon: Option<Pokemon> = None;

    let (pokedata, moves) = banish! {
        #![async]

        #[trace]
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

    println!("\n==== POKEDATA ====");
    if let Some(p) = &pokedata {
        println!("Name: {}", p.name);
        println!("Base experience: {}", p.base_experience);
        println!("Height: {}", p.height);
        println!("Weight: {}", p.weight);
    }

    println!("\nFirst 5 moves:");
    for m in &moves {
        println!("  - {}", m);
    }
}
```

`#![async]` makes the block expand to an `async move { ... }` expression, which is why `.await` is valid inside rule bodies and why the block itself must be `.await`ed by the caller. `load_pokemon?` and `load_pokemon_moves?` are conditionless rules, so each fires exactly once per state entry in declaration order. `pokemon` is declared outside the block so it can be mutated by `load_pokemon?` and then read and returned by `load_pokemon_moves?`. `#[trace]` on `@fetch_pokemon` emits a log entry on state entry and before each rule evaluation. Run with `$env:RUST_LOG="trace"; cargo run -q 2> trace.log` to capture output to a log file.

---

## Known Limitations

**Transitions cannot be used inside nested blocks or closures.** `=> @state` and `=> @state if condition` must appear as standalone statements inside a rule body or fallback branch. They cannot be used inside a nested `if`, closure, or other block within a rule body. For a single condition, use a guarded transition. For more complex branching, split the logic into separate conditional rules.

```rust
// Does not work
step? { if condition { => @other; } }

// Works: guarded transition handles the simple case
step? { => @other if condition; }

// Works: conditional rule handles anything else
step ? condition { => @other; }
```

**`banish!` cannot be used in `const` contexts.** The generated code uses mutable variables and loops, which are not const-evaluable.

**Rule names are identifiers, not strings.** They exist for readability and error messages only. They cannot be inspected or matched at runtime.