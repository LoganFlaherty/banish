# Banish
[![Crates.io](https://img.shields.io/crates/v/banish.svg)](https://crates.io/crates/banish)
[![Docs.rs](https://docs.rs/banish/badge.svg)](https://docs.rs/banish)
[![License](https://img.shields.io/crates/l/banish.svg)](https://github.com/LoganFlaherty/banish/blob/main/LICENSE)

Banish is a declarative framework for rule-based state machines in Rust. Define states and rules, and Banish deterministically resolves scheduling and transitions, delivering complex state management with zero runtime overhead.

```rust
use banish::banish;

// Will print all light colors twice
fn main() {
    banish! {
        let mut ticks: i32 = 0;

        // State attribute that limits state entries
        #[max_entry = 2]
        @red // State declaration
            // Conditionless rule that runs once while in red
            announce? {
                ticks = 0;
                println!("\nRed light");
            }

            // Causes @red to loop till false
            timer ? ticks < 3 {
                ticks += 1;
            }
        // @red finishes and transitions to @green

        @green
            announce? { println!("Green light"); }

            timer ? ticks < 6 {
                ticks += 1;
            }

        @yellow
            announce? { println!("Yellow light"); }

            timer ? ticks < 10 {
                ticks += 1;
            } !? { => @red; } // Explicit transition to @red as a fallback
    }
}
```

## Why Banish?
* **Generated Runtime:** Banish generates a fixed-point runtime at compile time and schedules your states through it. Rules fire when their conditions are met, not when you call them. The scheduler, interaction tracking, and state advancement are all handled for you.
* **Flexible Transitions:** States advance implicitly in declaration order by default. Explicit `=> @state` transitions let you jump anywhere when you need to.
* **Runtime Dispatch:** `#![dispatch(expr)]` and the `BanishDispatch` trait let external enums determine the entry state at runtime, making machines resumable and configurable without any extra wiring.
* **`no_std` Compatible:** Works in embedded and bare-metal environments. The `trace-logger` feature requires `std`; everything else does not.
* **Async-Runtime-Agnostic:** Works with tokio, async-std, smol, or anything else that can drive a future. No integration code required.
* **Full Rust Integration:** Rule bodies are plain Rust. Closures, external crates, mutable references. Everything works as you'd expect.
* **Self-Documenting Structure:** Named states and named rules make the shape of your logic readable at a glance, without requiring comments to explain what each block is doing.

## Comparison

Most state machines in Rust end up as a `loop` wrapping a `match` wrapping a pile of `if` chains with careful flag management. The structure of the problem gets lost in the structure of the code. Banish flips this around.

Here's the traffic light example from above written by hand:

```rust
// Without banish
fn main() {
    #[derive(PartialEq)]
    enum Light { Red, Green, Yellow }

    let mut ticks: i32 = 0;
    let mut state = Light::Red;
    let mut red_entries: usize = 0;
    let mut first_iteration = true;

    loop {
        match state {
            Light::Red => {
                if first_iteration {
                    if red_entries >= 2 { break; }
                    red_entries += 1;
                    ticks = 0;
                    println!("\nRed light");
                    first_iteration = false;
                }

                let mut interaction = false;
                if ticks < 3 { ticks += 1; interaction = true; }
                if !interaction { state = Light::Green; first_iteration = true; }
            }
            Light::Green => {
                if first_iteration {
                    println!("Green light");
                    first_iteration = false;
                }

                let mut interaction = false;
                if ticks < 6 { ticks += 1; interaction = true; }
                if !interaction { state = Light::Yellow; first_iteration = true; }
            }
            Light::Yellow => {
                if first_iteration {
                    println!("Yellow light");
                    first_iteration = false;
                }
                
                if ticks < 10 {
                    ticks += 1;
                } else {
                    state = Light::Red;
                    first_iteration = true;
                    continue;
                }
            }
        }
    }
}
```

The manual version requires you to declare the enum, wire up the entry counter, carry a `first_iteration` flag across states, track `interaction` in every arm, and advance the state yourself. The banish version is just the logic.

## Install

```toml
[dependencies]
banish = "1.4.1"
```

Or with cargo:

```
cargo add banish
```

## Concepts
* **States** (`@name`) group related rules. Banish evaluates states sequentially by default.
* **Rules** (`name ? condition { body }`) are the logic inside a state. Rules fire when their condition evaluates to true, triggering the state to re-evaluate from the top.
* **Pattern conditions** (`name ? let Pat = expr { body }`) use `if let` semantics. The rule fires when the pattern matches, binding variables into the rule body.
* **Variables** (`let`) can be declared at block level or at state level, which re-initialize on every entry.
* **Fallback branches** (`!? { body }`) run when the preceding rule's condition is false. Does not trigger re-evaluation on its own.
* **Conditionless rules** (`name ? { body }`) fire exactly once on the first pass of each state entry.
* **Transitions** use `=> @state` for explicit jumps or `=> @state if condition;` for guarded jumps that bypass the implicit scheduler.
* **Control flow** use `return expr;` to exit the entire machine block, `break;` to exit the current state, and `continue;` to force an immediate re-evaluation of the rules.

## State Attributes

Attributes go above a state declaration and modify its behavior.

```rust
#[isolate, max_iter = 10 => @fallback, trace]
@my_state
    ...
```

| Attribute | Description |
|---|---|
| `isolate` | Removes the state from implicit scheduling. Only reachable via explicit `=> @state` transition. Must have a defined exit path. |
| `max_iter = N` | Caps the fixed-point loop to N iterations, then advances normally. |
| `max_iter = N => @state` | Same, but transitions to `@state` on exhaustion instead of advancing. |
| `max_entry = N` | Limits how many times this state can be entered. Returns on the (N+1)th entry. |
| `max_entry = N => @state` | Same, but transitions to `@state` on exhaustion instead of returning. |
| `trace` | Emits diagnostics via `log::trace!` on state entry and before each rule evaluation. Requires a `log`-compatible backend (see below). |

## Block Attributes
 
Block attributes go at the top of a `banish!` block, before the first state, and modify the behavior of the entire block.
 
```rust
banish! {
    #![async]
 
    @my_state
        ...
}
```
 
| Attribute | Description |
|---|---|
| `async` | Expands the block to an `async move { ... }` expression. Required for `.await` inside rule bodies. The result must be `.await`ed by the caller. |
| `id = "name"` | Sets a display name included in all `trace` output for this machine, emitted as `[banish:name]` instead of `[banish]`. Has no effect if no states use `#[trace]`. |
| `dispatch(expr)` | Sets the entry state dynamically at runtime from an enum value. The enum must derive `BanishDispatch`. Variant names are matched to state names by converting PascalCase to snake_case. |
| `trace` | Enables trace diagnostics on every state in the block. Equivalent to placing `#[trace]` on each state individually. Requires a `log`-compatible backend (see below). |
 
## Dispatch
 
`#![dispatch(expr)]` replaces the fixed entry state with a runtime lookup, making machines resumable and configurable at the call site. The enum must derive `BanishDispatch`, which maps each variant to its snake_case state name with no runtime allocation.
 
```rust
use banish::BanishDispatch;
 
#[derive(BanishDispatch)]
enum PipelineState {
    Normalize,
    Validate,
    Finalize,
}
 
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
 
`PipelineState::Validate` maps to `"validate"` which matches `@validate`, so the machine enters there directly. Variants with data fields are supported. The data is ignored and only the variant name is used for dispatch. Passing a variant whose name does not match any state is a runtime panic.

## Function Attributes
 
Function attributes are declared on `fn` items and modify how the function interacts with its `banish!` block.
 
```rust
#[banish::machine]
async fn my_machine() {
    banish! { ... }
}
```
 
| Attribute | Description |
|---|---|
| `#[banish::machine]` | Setup attribute. Injects `async` and `.await` for async functions. Sets `id` to the function name. This is ignored if the items are already present. |
 
**Attribute ordering.** `#[banish::machine]` must come before any runtime attribute such as `#[tokio::main]`. Attributes apply top to bottom, so `#[banish::machine]` must see the original function before the runtime transforms it:
 
```rust
#[banish::machine]  // runs first, sees the original async fn
#[tokio::main]      // runs second, wraps the result in the runtime
async fn main() {
    banish! { ... }
}
```

## Tracing

The `trace` attribute emits diagnostics through the [`log`](https://docs.rs/log) facade. The simplest way to enable it is `banish::init_trace`, available behind the `trace-logger` feature:
 
```toml
[dependencies]
banish = { version = "1.4.1", features = ["trace-logger"] }
```
 
Call it once at the start of `main`. Pass `Some("file path")` to write output to a file, or pass `None` to print to stderr:
 
```rust
fn main() {
    banish::init_trace(Some("trace.log")); // write to file
    // banish::init_trace(None); // print to stderr
    ...
}
```
 
If you need full control over log routing or filtering, skip `init_trace` and initialise any `log`-compatible backend directly instead. Banish emits all trace diagnostics through the `log` facade, so any backend will capture them.

## More Examples

* The [Dragon Fight](https://github.com/LoganFlaherty/banish/blob/main/docs/reference.md#dragon-fight) example is a turn-based battle that demonstrates early return with a value, external crate usage, cycling transitions, and using the state attribute `max_iter` with the transition option.
* The [Async HTTP Fetch](https://github.com/LoganFlaherty/banish/blob/main/docs/reference.md#async-http-fetch) example is an async workflow that demonstrates `#![async, id = ""]`, `.await`, `#[trace]`, and returning a tuple value from an async block. Pokemon data is fetched from the `pokeapi` and loaded into structs to be accessed.
* The [Record Normalizer](https://github.com/LoganFlaherty/banish/blob/main/docs/reference.md#record-normalizer) example is an async multi-pass normalization pipeline that demonstrates `#[banish::machine]` and an isolated error state. Records are loaded from a file asynchronously, normalized in place, and written back out. If the load fails, an isolated `@error` state handles the failure and exits cleanly.
* The [Order Processing Pipeline](https://github.com/LoganFlaherty/banish/blob/main/docs/reference.md#order-processing-pipeline) example is a resumable order processing pipeline that demonstrates `#![dispatch(...)]`, `BanishDispatch`, state-level variables, guarded transitions, and conditionless rules. Orders can be resumed from any stage by dispatching into the pipeline with the appropriate `OrderStage` variant.

## Reference
For a deeper dive, see the [Reference](https://github.com/LoganFlaherty/banish/blob/main/docs/reference.md).

## Contributing

Contributions are welcome. Before opening a PR, please open a discussion first. This keeps design decisions visible and avoids duplicated effort.

The test suite covers all documented behavior and edge cases. Run it locally before submitting:

```
cargo test
```

New behavior and edge cases should include corresponding tests. Note that when writing error tests, the first test run fails and writes the error output into a `wip` directory. Those should be inspected for accuracy and then moved to the errors directory. Following test runs should pass.
