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
            announce ? {
                ticks = 0;
                println!("Red light");
            }
            timer ? ticks < 3 {
                ticks += 1;
            }

        @green
            announce ? { println!("Green light"); }
            timer ? ticks < 6 {
                ticks += 1;
            }

        @yellow
            announce ? { println!("Yellow light"); }
            timer ? ticks < 10 {
                ticks += 1;
            } !? { => @red; }
    }
}
```

## Why Banish?
* **Fixed-Point Looping:** States automatically re-evaluate their rules until none of them fire, then advance.
* **Zero Runtime Overhead:** Banish is a procedural macro. It generates standard optimized Rust at compile time. No interpreter, no allocations, no virtual machine.
* **Full Rust Integration:** Rule bodies are plain Rust. Closures, external crates, mutable references. Everything works as you'd expect.
* **Self-Documenting Structure**: Named states and named rules make the shape of your logic readable at a glance, without requiring comments to explain what each block is doing.
* **Flexible Transitions:** States advance implicitly in declaration order by default. Explicit `=> @state` transitions let you jump anywhere when you need to.

## Comparison
Most state machines in Rust end up as a `loop` wrapping a `match` wrapping a pile of `if` chains with careful flag management. The structure of the problem gets lost in the structure of the code. Banish flips this around. You write the *what*, not the *how*.

```rust
// Without banish
let mut state = 0;
loop {
    match state {
        0 => {
            let mut interaction = false;
            if !initialized {
                initialized = true;
                value = compute();
                interaction = true;
            }
            if initialized && value > threshold {
                value = clamp(value);
                interaction = true;
            }
            if initialized && value <= threshold {
                state = 1;
                interaction = true;
            }
            if !interaction { state = 1; }
        }
        1 => {
            let mut interaction = false;
            if !logged {
                logged = true;
                log(value);
                interaction = true;
            }
            if logged && errors.is_empty() { return Ok(value); }
            if logged && !errors.is_empty() { return Err(errors); }
            if !interaction { break; }
        }
        _ => unreachable!()
    }
}

// With banish
@normalize
    setup ? !initialized {
        initialized = true;
        value = compute();
    }
    clamp ? value > threshold { value = clamp(value); }

@report
    log ? !logged {
        logged = true;
        log(value);
    }
    finish ? errors.is_empty() { return Ok(value); }
    fail ? !errors.is_empty() { return Err(errors); }
```

The manual version has three concerns tangled together: state indexing, interaction tracking, and the actual logic. The banish version is just the logic.

## Concepts

**States** (`@name`) group related rules. The machine starts at the first declared state and advances through them in order.

**Rules** (`name ? condition { body }`) fire when their condition is true. After firing, the state re-evaluates from the top. Once a full pass completes with no rules firing, the state has reached its fixed point and the machine advances.

**Conditionless rules** (`name ? { body }`) fire exactly once per state entry, on the first pass.

**Fallback branches** (`!? { body }`) run when a rule's condition is false, every pass.

**Explicit transitions** (`=> @state;`) jump to any named state immediately, bypassing the implicit scheduler.

**Return values** (`return expr;`) work naturally. Exits the entire `banish!` block with a value, just like returning from a closure.

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
| `trace` | Prints state entry and rule evaluation to stderr. Useful for debugging. |

## Install

```toml
[dependencies]
banish = "1.2.0"
```

Or with cargo:

```
cargo add banish
```

## More Examples

See [`docs/README.md`](https://github.com/LoganFlaherty/banish/blob/main/docs/README.md) for more examples including game logic, search algorithms, and data pipelines.

## Contributing

Contributions are welcome. Before opening a PR, please open a discussion first. This keeps design decisions visible and avoids duplicated effort.

The test suite covers all documented behavior and edge cases. Run it locally before submitting:

```
cargo test
```

New behavior and edge cases should include corresponding tests.
