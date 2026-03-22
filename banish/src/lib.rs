/*!
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

```rust,ignore
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

```rust,ignore
@process
    clamp ? value > 100 { value = 100; }

    log ? ready { println!("{}", value); ready = false; }
```

### Conditionless Rules

```
name ? { body }
```

A conditionless rule fires exactly once per state entry, on the first pass only. It sets `__interaction = true` on that pass, causing the state to re-evaluate, but it will not fire again on subsequent passes within the same entry. Conditionless rules cannot have a fallback branch.

```rust,ignore
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

```rust,ignore
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

## Variables
 
Plain Rust `let` declarations can be placed at two scopes: directly inside the `banish!` block before any state, or inside a state before any rules. Both follow standard Rust `let` syntax including type annotations and `let mut`.
 
### Block-level
 
Block-level declarations appear after the `#![...]` attribute block (if present) and before the first state. They are emitted inside the generated closure or async block and live for the entire lifetime of the machine, accessible from every state and rule.
 
```rust,ignore
banish! {
    let mut count: u32 = 0;
    let threshold = 10;
 
    @accumulate
        inc ? count < threshold { count += 1; }
 
        done ? count >= threshold { return count; }
}
```
 
Variables declared here behave identically to variables declared outside the block and captured by move, but keep the declaration co-located with the logic that uses it.

### State-level
 
State-level declarations appear after the `@name` line and before any rules. They are re-initialized on every entry to the state, making them suitable for per-pass scratch values that do not need to persist across entries.
 
```rust,ignore
banish! {
    @process
        let mut dirty = false;
 
        check_a ? condition_a { dirty = true; }

        check_b ? condition_b { dirty = true; }
 
        flush ? dirty {
            write_output();
            return;
    }
}
```
 
If a value needs to persist across entries to the same state, declare it at block level instead.
 
**Shadowing:** a state-level declaration with the same name as a block-level one is valid Rust and shadows the outer binding within that state's scope. The block-level variable is unaffected.

---

## Transitions

### Implicit (Scheduler)

Once a state reaches its fixed point, the machine automatically advances to the next non-isolated state in declaration order. This is the default behavior and requires no syntax.

### Explicit (`=> @state`)

```
=> @state;
```

An explicit transition immediately jumps to the named state, bypassing the implicit scheduler. It must appear as a standalone statement in a rule body or fallback branch. The remaining rules in the current pass are abandoned and the target state begins a fresh evaluation.

```rust,ignore
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

```rust,ignore
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

```rust,ignore
@process
    step ? !done { do_work(); }

    bail ? error  { break; }   // exit state early, no transition needed
```

---

## Return Values

`return` exits the entire `banish!` block immediately, optionally with a value. The block evaluates to that value, so it can be assigned or returned from an enclosing function.
 
```rust,ignore
let result: &str = banish! {
    @grade
        pass ? score >= 60 {
            return "pass";
        } !? { return "fail"; }
};
```
 
The return type is inferred by the Rust compiler from the `return` expressions in the block. If the block returns nothing, it evaluates to `()`.
 
When `banish!` is the tail expression of a function, its value is the function's return value implicitly. `return` inside the block exits the generated closure, and the closure's value becomes the function's result.
 
```rust,ignore
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

```rust,ignore
#[isolate, max_iter = 10 => @fallback, trace]
@my_state
    // Rules
    ...
```

---

### `isolate`

Removes the state from implicit scheduling. An isolated state is never entered by the scheduler. It can only be reached via an explicit `=> @state` transition.

```rust,ignore
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

```rust,ignore
#[max_iter = 5]
@process
    step ? !done { do_work(); }
```

Useful as a safety net on states that could theoretically loop many times, or to limit processing per scheduler pass.

---

### `max_iter = N => @state`

Same as `max_iter = N`, but instead of advancing normally on exhaustion, the machine transitions to the named state.

```rust,ignore
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

```rust,ignore
#[max_entry = 2]
@red
    announce? { println!("Red light"); ticks = 0; }

    timer ? ticks < 3 { ticks += 1; }
```

In the traffic light example above, the machine exits after `@red` is entered a third time. The entry counter persists for the lifetime of the `banish!` block and is not reset between cycles.

**Note:** `max_entry = N => @state` on an isolated state does not remove the requirement for a rule-level exit. The redirect only fires on the (N+1)th entry. Entries 1 through N still need a `return`, `=> @state`, or `max_iter = N => @state` to exit normally.

---

### `trace`

Emits diagnostics via [`log::trace!`](https://docs.rs/log) on state entry and before each rule evaluation. Requires a backend to capture output. Without one, diagnostics are silently discarded.
 
```rust,ignore
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
 
The simplest way to enable trace output is `banish::init_trace`, available behind the `trace-logger` feature:
 
```toml
[dependencies]
banish = { version = "1.x", features = ["trace-logger"] }
```
 
Call it once at the start of `main`. Pass `Some("file path")` to write output to a file, or pass `None` to print to stderr:
 
```rust,ignore
fn main() {
    banish::init_trace(Some("trace.log")); // write to file
    // banish::init_trace(None); // print to stderr
    ...
}
```
 
**Custom backends:** If you need full control over log routing or filtering, skip `init_trace` and initialise any [`log`](https://docs.rs/log)-compatible backend directly. Banish emits all trace diagnostics through the `log` facade, so any backend will capture them. `log` is re-exported from `banish` and does not need to be added as a separate dependency.

---

## Block Attributes
 
Block attributes are declared at the top of a `banish!` block using inner attribute syntax and modify the behavior of the entire block. Multiple attributes are comma-separated.
 
```rust,ignore
banish! {
    #![async, id = "fetcher"]
 
    @my_state
        ...
}
```
 
The `#![...]` line must appear before the first state declaration. Only one `#![...]` block is permitted per `banish!` invocation.

**Combining with state attributes:** block attributes are independent of state-level attributes. Meaning they all work normally in any combination.
 
---
 
### `async`
 
Expands the `banish!` block to an `async move { ... }` expression instead of an immediately invoked closure. The result is a `Future` and must be `.await`ed by the caller.
 
```rust,ignore
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
 
```rust,ignore
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

---

### `id = "name"`

Sets a display name for this machine that is included in all `trace` output. Without `id`, trace lines are prefixed with `[banish]`. With `id`, they are prefixed with `[banish:name]`.

```rust,ignore
banish! {
    #![id = "lexer"]

    #[trace]
    @tokenize
        next ? !done { advance(); }

        finish ? done { return; }
}
```

Output:
```
[banish:lexer] entering state `tokenize`
[banish:lexer] rule `next`: condition = true
[banish:lexer] rule `finish`: condition = false
```

This is most useful when multiple `banish!` blocks emit trace output in the same run. Without an `id`, their output is indistinguishable. `id` has no effect if no states in the block use `#[trace]`.

---

### `dispatch(expr)`
 
Sets the entry state dynamically at runtime by matching the variant name of `expr` against the declared state names. The entry state is no longer fixed at compile time. It is resolved on each invocation from the value of `expr`.
 
```rust,ignore
let entry = PipelineState::Validate;
banish! {
    #![dispatch(entry)]
 
    @normalize
        ...
 
    @validate
        ...
 
    @finalize
        done? { return; }
}
```
 
The enum passed to `dispatch` must implement `BanishDispatch`, which maps each variant to its snake_case name as a `&'static str`. The simplest way to satisfy this is `#[derive(BanishDispatch)]`. See [BanishDispatch](#banishdispatch) for details.
 
**Variant name matching** converts PascalCase variant names to snake_case and matches them against state names verbatim. `Normalize` matches `@normalize`, `FetchPokemon` matches `@fetch_pokemon`, and so on. Passing a variant whose converted name does not match any declared state is a runtime panic.
 
**Variants with data** are supported. The data is ignored. Only the variant name is used for dispatch. If you need the data, extract it before the block:
 
```rust,ignore
let payload = match &entry {
    PipelineState::Resume(data) => Some(data),
    _ => None,
};
 
banish! {
    #![dispatch(entry)]
    ...
}
```
 
**Combining with other block attributes** works normally. `dispatch` can appear alongside `async` and `id` in the same `#![...]` line:
 
```rust,ignore
banish! {
    #![async, id = "pipeline", dispatch(entry)]
    ...
}
```

---

### `trace`
 
Enables trace diagnostics on every state in the block. Equivalent to placing `#[trace]` on each state individually. See the [`trace`](#trace) state attribute for output format and backend setup.
 
```rust,ignore
banish! {
    #![trace, id = "pipeline"]
 
    @normalize
        ...
 
    @finalize
        ...
}
```
 
State-level `#[trace]` continues to work alongside `#![trace]` and is redundant but not an error.

---

## BanishDispatch
 
`BanishDispatch` is a trait used by `#![dispatch(expr)]` to resolve a variant name at runtime. It has a single method:
 
```rust,ignore
pub trait BanishDispatch {
    fn variant_name(&self) -> &'static str;
}
```
 
`variant_name` returns the snake_case name of the current variant as a `&'static str`. The return type is static so there is no allocation on dispatch.
 
### Deriving
 
`#[derive(BanishDispatch)]` generates the implementation automatically. All variant kinds are supported. Unit, tuple, and struct variants all produce the correct name. Data fields are ignored.
 
```rust,ignore
use banish::BanishDispatch;
 
#[derive(BanishDispatch)]
enum PipelineState {
    Normalize,
    Validate,
    Finalize,
    Resume(CheckpointData),   // data is ignored, name still matches @resume
}
```
 
### Manual Implementation
 
If you need custom naming or are not using the derive macro:
 
```rust,ignore
impl BanishDispatch for PipelineState {
    fn variant_name(&self) -> &'static str {
        match self {
            PipelineState::Normalize => "normalize",
            PipelineState::Validate => "validate",
            PipelineState::Finalize => "finalize",
            PipelineState::Resume(_) => "resume",
        }
    }
}
```
 
The returned string must match a declared state name exactly. Returning a name that does not match any state causes a runtime panic on dispatch.

---

## Function Attributes
 
Function attributes are declared on `fn` items using outer attribute syntax and modify how the function interacts with its `banish!` block. They are distinct from block attributes, which are written inside the `banish!` block with `#![...]`.
 
---
 
### `#[banish::machine]`
 
A setup attribute that reduces boilerplate for functions whose body contains a `banish!` block. It does three things automatically:
 
**Injects `async` into the block attribute** when applied to an `async fn`, so `#![async]` does not need to be written manually. Writing it explicitly is also fine. The attribute detects it and skips injection.

**Injects `.await` on the `banish!` expression** when the function is async, so the future produced by `#![async]` is driven to completion automatically. If `.await` is already present it is left alone.
 
**Sets `id` to the function name** so trace output is labelled without any extra boilerplate. Can be overridden by writing `#![id = "name"]` inside the `banish!` block explicitly.
 
Injections don't happen if the corresponding item is already present. `#[banish::machine]` is purely additive.
 
```rust,ignore
// Before
#[tokio::main]
async fn normalizer() {
    banish! {
        #![async, id = "normalizer"]
 
        @process
            step? { do_work().await; return; }
    }
}
 
// After
#[banish::machine]
#[tokio::main]
async fn normalizer() {
    banish! {
        @process
            step? { do_work().await; return; }
    }
}
```
 
**Attribute ordering.** `#[banish::machine]` must come before any runtime attribute such as `#[tokio::main]`. Attributes apply top to bottom. `#[banish::machine]` must see the original `async fn` before the runtime transforms it, otherwise it cannot locate the `banish!` block.
 
```rust,ignore
#[banish::machine]  // must be first
#[tokio::main]
async fn main() {
    banish! { ... }
}
```
 
**Placement of `banish!`.** The `banish!` invocation can appear anywhere in the function body as a standalone statement, a tail expression, or a `let` binding:
 
```rust,ignore
#[banish::machine]
#[tokio::main]
async fn main() {
    setup();
    let result = banish! {
        @grade
            pass ? score >= 60 { return "pass"; } !? { return "fail"; }
    };
    println!("{}", result);
}
```
 
**`#[banish::machine]` takes no arguments.** All block-level configuration belongs inside `#![...]` within the `banish!` block, exactly as it does without the attribute. The function attribute only handles the two injections described above.

---

## Examples

### Traffic Lights

A simple state machine that demonstrates implicit and explicit state transition, block variables, conditionless rules, fallback branches, and using the state attribute `max_entry`.

```rust,ignore
use banish::banish;

fn main() {
    banish! {
        let mut ticks: i32 = 0;

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

`@red` and `@green` each reach their fixed point once their timer rule stops firing.

`@yellow`'s fallback branch transitions back to `@red` explicitly. Without this, fallback branches do not trigger re-evaluation.

`max_entry = 2` ensures this machine only loops through all its states twice. Immediately returning on the third entry of `@red`.

---

### Dragon Fight

A turn-based battle that demonstrates early return with a value, external crate usage, cycling transitions, and using the state attribute `max_iter` with the transition option.

```toml
[dependencies]
rand = "0.10.0"
```

```rust,ignore
use banish::banish;
use rand::prelude::*;

fn main() {
    println!("\n==== Battle Start ====");

    let result: &str = banish! {
        let mut rng = rand::rng();
        let mut player_hp = 20;
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

    println!("GAME OVER: {}", result);
}
```

`max_iter = 1` on each state caps the fixed-point loop to a single iteration.

After `attack?` fires and `check_win` or `check_loss` has been evaluated, if neither returns, the iteration limit is exhausted and the redirect transitions to the opposing state.
 
---

### Async HTTP Fetch

An async workflow that demonstrates `#![async, id = ""]`, `.await`, `#[trace]`, and returning a tuple value from an async block. Pokemon data is fetched from the `pokeapi` and loaded into stucts to be accessed.

```toml
[dependencies]
banish = { version = "1.3.0", features = ["trace-logger"] }
tokio = { version = "1.50.0", features = ["full"] }
reqwest = { version = "0.13.2", features = ["json"] }
serde = { version = "1.0.288", features = ["derive"] }
env_logger = "0.11.9"
```

```rust,ignore
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
    banish::init_trace(Some("trace.log"));
    let (pokedata, moves) = banish! {
        #![async, id = "pokedata"]

        let mut pokemon: Option<Pokemon> = None;

        #[trace]
        @fetch_pokemon
            load_pokemon? {
                let response = reqwest::get("https://pokeapi.co/api/v2/pokemon/charizard")
                    .await
                    .expect("Request failed");
                pokemon = Some(
                    response.json::<Pokemon>().await.expect("failed to parse pokemon")
                );
            }

            load_pokemon_moves? {
                let response = reqwest::get("https://pokeapi.co/api/v2/pokemon/charizard")
                    .await
                    .expect("Request failed");
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

`#![async]` makes the block expand to an `async move { ... }` expression, which is why `.await` is valid inside rule bodies and why the block itself must be `.await`ed by the caller.

`load_pokemon?` and `load_pokemon_moves?` are conditionless rules, so each fires exactly once per state entry in declaration order.

`pokemon` is declared outside the block so it can be mutated by `load_pokemon?` and then read and returned by `load_pokemon_moves`.

`#[trace]` on `@fetch_pokemon` emits a log entry on state entry and before each rule evaluation.

---

### Record Normalizer
 
An async multi-pass normalization pipeline that demonstrates `#[banish::machine]` and an isolated error state. Records are loaded from a file asynchronously, normalized in place, and written back out. If the load fails, an isolated `@error` state handles the failure and exits cleanly.
 
```toml
[dependencies]
banish = "1.3.0"
tokio = { version = "1.50.0", features = ["full"] }
```
 
```rust,ignore
use banish::banish;
 
#[banish::machine]
#[tokio::main]
async fn main() {
    banish! {
        let mut records: Vec<String> = Vec::new();
        let mut load_error: Option<String> = None;
 
        @fetch
            load? {
                match tokio::fs::read_to_string("../records.txt").await {
                    Ok(content) => {
                        records = content.lines().map(str::to_string).collect();
                    }
                    Err(e) => {
                        load_error = Some(e.to_string());
                    }
                }
            }

            // Transition is a standalone statement here because transitions
            // cannot appear inside nested blocks such as match arms.
            bail ? load_error.is_some() { => @error; }
 
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
                let output = records.join("\n");
                tokio::fs::write("../records_clean.txt", output).await.expect("Write failed");
                println!("Wrote {} records to records_clean.txt", records.len());
                return;
            }
 
        #[isolate]
        @error
            handle? {
                eprintln!("Failed to load records: {}", load_error.unwrap());
                return;
            }
    }
}
```
 
`#[banish::machine]` injects `async` and `id = "normalize_records"` into the block attribute automatically, and drives the resulting future to completion with `.await`. Neither needs to be written by hand.
 
`@fetch` loads the file and stores any error in `load_error` rather than transitioning directly from inside the match arm, which is not permitted.

`bail` reads the flag and transitions to @error if it is set. This is the standard pattern for routing to an isolated state from within a conditionless rule that does async work.
 
`@error` is isolated, so it is never entered by the implicit scheduler. It can only be reached via the explicit `=> @error` transition in `@fetch`. It must have a defined exit, in this case a `return`, because an isolated state with no exit is a compile error.
 
`@normalize` re-evaluates until all three rules stop firing, converging when the records are fully trimmed, lowercased, and non-empty. `@finalize` sorts, deduplicates, and writes the result back to disk on a single conditionless pass, then returns.

---

### Order Processing Pipeline
 
A resumable order processing pipeline that demonstrates `#![dispatch(...)]`, `BanishDispatch`, a state-level variable, a guarded transition to an isolated error state.
 
```rust,ignore
use banish::banish;
use banish::BanishDispatch;
 
#[derive(BanishDispatch)]
enum OrderStage {
    Validate,
    ApplyDiscounts,
    Finalize,
}
 
struct LineItem {
    name: &'static str,
    quantity: u32,
    unit_price_cents: u32,
}
 
struct Order {
    id: &'static str,
    customer: &'static str,
    items: Vec<LineItem>,
    coupon: Option<&'static str>,
}
 
fn process_order(order: Order, resume_from: OrderStage) {
    banish! {
        #![dispatch(resume_from)]
 
        let subtotal_cents: u32 = order.items.iter()
            .map(|i| i.quantity * i.unit_price_cents)
            .sum();
        let mut total_cents: u32 = subtotal_cents;
        let mut discount_cents: u32 = 0;
        let mut rejected: Option<&str> = None;
 
        // Validate that every line item has a non-zero quantity and price.
        // Uses a state-level variable to track which item is currently being
        // checked so the rule can scan one item per pass.
        @validate
            let mut idx: usize = 0;
 
            check ? idx < order.items.len() {
                let item = &order.items[idx];
                if item.quantity == 0 || item.unit_price_cents == 0 {
                    rejected = Some(item.name);
                }
                idx += 1;
            }
 
            // Jump to @rejected if any item failed validation, otherwise
            // fall through to @apply_discounts normally.
            route? {
                => @rejected if rejected.is_some();
            }
 
        @apply_discounts
            // Apply a flat 10% loyalty discount for orders over $100.
            loyalty? {
                if subtotal_cents >= 10_000 {
                    let loyalty_discount = subtotal_cents / 10;
                    discount_cents += loyalty_discount;
                    println!("  Loyalty discount: -${:.2}", loyalty_discount as f64 / 100.0);
                }
            }
 
            // Apply coupon. "SAVE20" takes 20% off, "FIVE" takes $5 off.
            coupon? {
                if let Some(code) = order.coupon {
                    let savings = match code {
                        "SAVE20" => subtotal_cents / 5,
                        "FIVE" => 500_u32.min(subtotal_cents),
                        other => { println!("  Unknown coupon: {}", other); 0 }
                    };
                    discount_cents += savings;
                    println!("  Coupon {}: -${:.2}", code, savings as f64 / 100.0);
                }
            }
 
            apply? {
                total_cents = subtotal_cents.saturating_sub(discount_cents);
            }
 
        @finalize
            receipt? {
                println!("\n  Order {}  --  {}", order.id, order.customer);
                println!("  ----------------------------------------");
                for item in &order.items {
                    let line = item.quantity * item.unit_price_cents;
                    println!(
                        "  {:<20} x{}  ${:.2}",
                        item.name, item.quantity, line as f64 / 100.0
                    );
                }
                println!("  ----------------------------------------");
                if discount_cents > 0 {
                    println!("  Subtotal:              ${:.2}", subtotal_cents as f64 / 100.0);
                    println!("  Discounts:            -${:.2}", discount_cents as f64 / 100.0);
                }
                println!("  Total:                 ${:.2}", total_cents as f64 / 100.0);
                return;
            }
 
        #[isolate]
        @rejected
            handle? {
                eprintln!(
                    "\n  Order {} rejected: invalid line item {:?}",
                    order.id,
                    rejected.unwrap()
                );
                return;
            }
    }
}
 
fn main() {
    println!("=== Full pipeline ===");
    process_order(
        Order {
            id: "ORD-001",
            customer: "Alice",
            items: vec![
                LineItem { name: "Mechanical Keyboard", quantity: 1, unit_price_cents: 8999 },
                LineItem { name: "USB-C Cable", quantity: 3, unit_price_cents:  999 },
                LineItem { name: "Desk Mat", quantity: 1, unit_price_cents: 2499 },
            ],
            coupon: Some("SAVE20"),
        },
        OrderStage::Validate,
    );
 
    println!("\n=== Resume from ApplyDiscounts (validation already passed) ===");
    process_order(
        Order {
            id: "ORD-002",
            customer: "Bob",
            items: vec![
                LineItem { name: "Monitor", quantity: 1, unit_price_cents: 29999 },
                LineItem { name: "HDMI Cable", quantity: 2, unit_price_cents:  1499 },
            ],
            coupon: Some("FIVE"),
        },
        OrderStage::ApplyDiscounts,
    );
 
    println!("\n=== Invalid order (zero quantity) ===");
    process_order(
        Order {
            id: "ORD-003",
            customer: "Charlie",
            items: vec![
                LineItem { name: "Webcam", quantity: 0, unit_price_cents: 5999 },
                LineItem { name: "Headset", quantity: 1, unit_price_cents: 7999 },
            ],
            coupon: None,
        },
        OrderStage::Validate,
    );
 
    println!("\n=== Jump straight to Finalize ===");
    process_order(
        Order {
            id: "ORD-004",
            customer: "Diana",
            items: vec![
                LineItem { name: "Mousepad", quantity: 1, unit_price_cents: 1999 },
            ],
            coupon: None,
        },
        OrderStage::Finalize,
    );
}
```
 
`#![dispatch(resume_from)]` selects the entry state at runtime from the `OrderStage` variant passed in. `#[derive(BanishDispatch)]` generates the `variant_name` implementation that maps each variant to its snake_case state name with no runtime allocation. `OrderStage::ApplyDiscounts` maps to `"apply_discounts"`, which matches `@apply_discounts` directly.
 
`@validate` uses a state-level `idx` to scan one item per pass. Because `idx` is declared at state level it resets to zero on every entry, making the scan restartable. The `route?` rule uses a guarded transition to jump to the isolated `@rejected` state if any item failed -- because `rejected` is set inside a nested `if` block, the transition must be a separate standalone rule rather than inside the `check` body.
 
`@apply_discounts` uses conditionless rules for `loyalty?`, `coupon?`, and `apply?`. Each needs to fire exactly once regardless of the result, so a conditionless rule is the right shape. A conditional rule would loop indefinitely because the conditions (`subtotal_cents >= 10_000`, `order.coupon.is_some()`) never change between passes.
 
`@rejected` is isolated, so it is never entered by the implicit scheduler. It can only be reached via the explicit `=> @rejected` transition in `route?`.
 
The four calls in `main` exercise every dispatch entry point and both exit paths. `@finalize` for success and `@rejected` for the invalid order.

---

## Known Limitations

**Transitions cannot be used inside nested blocks or closures.** `=> @state` and `=> @state if condition` must appear as standalone statements inside a rule body or fallback branch. They cannot be used inside a nested `if`, closure, or other block within a rule body. For a single condition, use a guarded transition. For more complex branching, split the logic into separate conditional rules.

```rust,ignore
// Does not work
step? { if condition { => @other; } }

// Works: guarded transition handles the simple case
step? { => @other if condition; }

// Works: conditional rule handles anything else
step ? condition { => @other; }
```

**`banish!` cannot be used in `const` contexts.** The generated code uses mutable variables and loops, which are not const-evaluable.

**Rule names are identifiers, not strings.** They exist for readability and error messages only. They cannot be inspected or matched at runtime.
*/

mod banish_dispatch;

pub use banish_derive::{ banish, machine, BanishDispatch };
pub use banish_dispatch::BanishDispatch;
pub use log;
use std::fs::File;

/// Initialises banish's built-in trace logger.
///
/// This is a convenience wrapper around [`env_logger`] that configures trace-level
/// logging for banish without requiring any environment variables or manual logger
/// setup. Call this once at the start of `main` before any `banish!` blocks run.
///
/// # Arguments
///
/// * `file_path` - If `Some`, trace output is written to the file at the given path.
///   The file is created if it does not exist and truncated if it does.
///   If `None`, output is written to stderr.
///
/// # Panics
///
/// Panics if `file_path` is `Some` and the file cannot be created, or if a global
/// logger has already been set by another call to this function or an external crate.
///
/// # Examples
///
/// Print trace output to stderr:
/// ```rust
/// banish::init_trace(None);
/// ```
///
/// Write trace output to a file:
/// ```rust
/// banish::init_trace(Some("trace.log"));
/// ```
///
/// If you need more control over log routing or filtering, skip this function and
/// initialise your own [`log`]-compatible backend instead. Banish emits all trace
/// diagnostics through the [`log`] facade, so any backend will capture them.
#[cfg(feature = "trace-logger")]
pub fn init_trace(file_path: Option<&str>) {
    let mut builder = env_logger::Builder::new();
    builder.filter_level(log::LevelFilter::Trace);

    if let Some(path) = file_path {
        let file = File::create(path).expect("banish: could not open trace file");
        builder.target(env_logger::Target::Pipe(Box::new(file)));
    }

    builder.init();
}