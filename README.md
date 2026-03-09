# Banish
[![Crates.io](https://img.shields.io/crates/v/banish.svg)](https://crates.io/crates/banish)
[![Docs.rs](https://docs.rs/banish/badge.svg)](https://docs.rs/banish)
[![License](https://img.shields.io/crates/l/banish.svg)](https://github.com/LoganFlaherty/banish/blob/main/LICENSE)

Banish is a declarative DSL for building rule-based state machines in Rust. States evaluate their rules until reaching a fixed point or triggering a transition, reducing control flow boilerplate.

```rust
use banish::banish;

// Will print all light colors twice
fn main() {
    let mut ticks: i32 = 0;
    banish! {
        // Returns on the third entry immediately
        #[max_entry = 2]
        @red
            announce? {
                ticks = 0;
                println!("Red light");
            }
            timer ? ticks < 3 {
                ticks += 1;
            }

        @green
            announce? { println!("Green light"); }
            timer ? ticks < 6 {
                ticks += 1;
            }

        @yellow
            announce? { println!("Yellow light"); }
            timer ? ticks < 10 {
                ticks += 1;
            } !? { => @red; }
    }
}
```

## Install

```toml
[dependencies]
banish = "1.2.2"
```

Or with cargo:

```
cargo add banish
```

## Why Banish?
* **Fixed-Point Looping:** States automatically re-evaluate their rules until none of them fire, then advance.
* **Zero Runtime Overhead:** Banish is a procedural macro. It generates standard optimized Rust at compile time. No interpreter, no allocations, no virtual machine.
* **Full Rust Integration:** Rule bodies are plain Rust. Closures, external crates, mutable references. Everything works as you'd expect.
* **Self-Documenting Structure**: Named states and named rules make the shape of your logic readable at a glance, without requiring comments to explain what each block is doing.
* **Flexible Transitions:** States advance implicitly in declaration order by default. Explicit `=> @state` transitions let you jump anywhere when you need to.

## Comparison
Most state machines in Rust end up as a `loop` wrapping a `match` wrapping a pile of `if` chains with careful flag management. The structure of the problem gets lost in the structure of the code. Banish flips this around. You write the *what*, not the *how*.

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
                    println!("Red light");
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

## Concepts

**States** (`@name`) group related rules. The machine starts at the first declared state and advances through them in order.

**Rules** (`name ? condition { body }`) fire when their condition is true. After firing, the state re-evaluates from the top. Once a full pass completes with no rules firing, the state has reached its fixed point and the machine advances.

**Conditionless rules** (`name ? { body }`) fire exactly once per state entry, on the first pass.

**Fallback branches** (`!? { body }`) run when a rule's condition is false, every pass.

**Explicit transitions** (`=> @state;`) jump to any named state immediately, bypassing the implicit scheduler.

**Return values** (`return expr;`) work naturally. Exits the entire `banish!` block with a value, just like returning from a closure.

**Early exit** (`break;` / `continue;`) work natively inside rule bodies against the generated fixed-point loop. `break` exits the current state and lets the scheduler advance normally. `continue` restarts rule evaluation from the top immediately.

## State Attributes

Attributes go above a state declaration and modify its behavior.

```rust
#[isolate, max_iter = 10 => @fallback, trace]
@my_state
    ...
```

| Attribute | Description |
|---|---|
| `isolate` | Removes the state from implicit scheduling. Only reachable via explicit `=> @state` transition. |
| `max_iter = N` | Caps the fixed-point loop to N iterations, then advances normally. |
| `max_iter = N => @state` | Same, but transitions to `@state` on exhaustion instead of advancing. |
| `max_entry = N` | Limits how many times this state can be entered. Returns on the (N+1)th entry. |
| `max_entry = N => @state` | Same, but transitions to `@state` on exhaustion instead of returning. |
| `trace` | Emits diagnostics via `log::trace!` on state entry and before each rule evaluation. Requires a `log`-compatible backend (see below). |

## Tracing

The `trace` attribute emits diagnostics through the [`log`](https://docs.rs/log) facade, giving you full control over where the output goes. `env_logger` is the simplest backend:

```toml
[dependencies]
env_logger = "0.11.9"
```

```rust
fn main() {
    env_logger::init();
    // ...
}
```

Then run with `RUST_LOG=trace` to capture output:

```powershell
# PowerShell
$env:RUST_LOG="trace"; cargo run -q 2> trace.log
```

```bash
# bash / zsh
RUST_LOG=trace cargo run -q 2> trace.log
```

## More Examples

The [Dragon Fight](https://github.com/LoganFlaherty/banish/blob/main/docs/README.md#dragon-fight) example demonstrates early return with a value, multi-state transitions, and external crate usage. The [Double For Loop](https://github.com/LoganFlaherty/banish/blob/main/docs/README.md#double-for-loop) example shows self-transitions and returning a tuple.

For a full treatment of every feature, attribute, and error, see the [Reference](https://github.com/LoganFlaherty/banish/blob/main/docs/reference.md).

## Contributing

Contributions are welcome. Before opening a PR, please open a discussion first. This keeps design decisions visible and avoids duplicated effort.

The test suite covers all documented behavior and edge cases. Run it locally before submitting:

```
cargo test
```

New behavior and edge cases should include corresponding tests.