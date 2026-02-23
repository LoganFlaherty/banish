## Banish
[![Crates.io](https://img.shields.io/crates/v/banish.svg)](https://crates.io/crates/banish)
[![Docs.rs](https://docs.rs/banish/badge.svg)](https://docs.rs/banish)
[![License](https://img.shields.io/crates/l/banish.svg)](https://github.com/LoganFlaherty/banish/blob/main/LICENSE)

Banish is a declarative DSL for building rule-based state machines in Rust. States evaluate their rules until reaching a fixed point or triggering a transition, reducing control flow boilerplate.

## Why Banish?
- Fixed-Point Looping: Unlike a standard function that executes top-to-bottom once, states loop automatically until no rules are triggered.
- Zero Runtime Overhead: Banish is a procedural macro. It generates standard, optimized Rust code at compile time. There is no interpreter or virtual machine.
- Mix Standard Rust: The body of every rule is just standard Rust code. You don't have to learn a whole new language, just a new structure.
- Self-Documenting: Banish structures your code into named States and Rules. This lets your code be understandable to other developers (or yourself six months later) without too much additional commenting.

## Features
- @States: Group logic into distinct states (e.g., @init, @process, @report).
- Rules?: Rules execute only when their condition is true (e.g., increment ? tick < 120).
- Fallback Branches!?: Provide alternate logic when a rule's condition is false.
- Fixed-Point Evaluation: When a rule executes, the state re-evaluates. Evaluation continues until no rules trigger.
- Implicit & Explicit Transitions: States transition in declaration order by default. Use **=> @state** for explicit jumps.
- Full Rust Integration: Rules have access to outer-scope variables, functions, and external crates.

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
banish = "1.1.5"
```

## Contributions
Contributions are welcome.

Before opening a PR, please start a discussion outlining your proposed changes. This helps ensure alignment on design decisions and prevents duplicated effort.

The test suite includes all documented examples. Please run the tests locally before submitting a PR.

If your changes introduce new behavior or address edge cases, include corresponding tests. Additional coverage for missing or unclear cases is appreciated.
