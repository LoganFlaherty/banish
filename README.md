## Banish
[![Crates.io](https://img.shields.io/crates/v/banish.svg)](https://crates.io/crates/banish)
[![Docs.rs](https://docs.rs/banish/badge.svg)](https://docs.rs/banish)
[![License](https://img.shields.io/crates/l/banish.svg)](https://github.com/LoganFlaherty/banish/blob/main/LICENSE)

Banish is a declarative DSL for building rule-based state machines in Rust. It allows you to define states and rules that execute until they reach a stable fixed point or trigger transitions, making complex control flow easier to express and reason about.

## Why Banish?
- Fixed-Point Solving: Unlike a standard function that runs top-to-bottom once, a Banish state loops internally until no more rules trigger. This makes it perfect for layout engines, constraint solvers, or complex game logic.
- Zero Runtime Overhead: Banish is a procedural macro. It generates standard, optimized Rust code at compile time. There is no interpreter or virtual machine.
- Mix Standard Rust: The body of every rule is just standard Rust code. You don't have to learn a whole new language, just a new structure.
- Organization: Writing complex state machines in raw Rust often leads to "spaghetti code" full of nested if/else, loop, and match blocks. Banish provides a clean, readable syntax to organize this logic.
- Self-Documenting: Banish structures your code into named Phases and Rules. This lets your code be instantly understandable to other developers (or yourself six months later) without too much additional commenting.

## Features
- @States: Group logic into distinct states (e.g., @init, @process, @report).
- Rules?: Rules that only execute when a condition is met (e.g., increment ? tick < 120).
- Convergence Loops: If a rule is triggered, the state automatically re-evaluates to ensure consistency.
- Automatic State Transitions: Once a state reaches a fixed point it transitions to the next state. However, the => @state syntax offers explicit transitions to any state.
- Scope Awareness: Variables, functions, and crates declared in your outer scope are available inside the DSL, making it easy to integrate into existing projects.

## Example
```rust
use banish::banish;

fn main() {
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
```

See more examples here: https://github.com/LoganFlaherty/banish/blob/main/docs/README.md

## Install
### Cargo
```
cargo add banish
```

### TOML
```
[dependencies]
banish = "1.1.4"
```
