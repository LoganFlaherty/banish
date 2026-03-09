//! # Banish
//! Banish is a declarative DSL for building rule-based state machines in Rust.
//! States evaluate their rules until reaching a fixed point or triggering a transition,
//! reducing control flow boilerplate.
//!
//! ## How It Works
//! A `banish!` block contains one or more states. The machine starts at the first declared
//! state and advances through them in declaration order. Within each state, rules are
//! evaluated top to bottom on every pass. If any rule fires, the state loops and re-evaluates
//! from the top. Once a full pass completes with no rules firing, the state has reached its
//! fixed point and the machine advances to the next state.
//!
//! ## Syntax
//!
//! | Syntax | Description |
//! |---|---|
//! | `@name` | Declares a state. |
//! | `rule ? condition { }` | A conditional rule. Fires when `condition` is true, then re-evaluates the state. |
//! | `rule ? { }` | A conditionless rule. Fires exactly once on the first pass of each state entry. |
//! | `!? { }` | Fallback branch. Runs when the preceding rule's condition is false. |
//! | `=> @state;` | Explicit transition. Immediately jumps to another state, bypassing the scheduler. |
//! | `return expr;` | Exits the entire `banish!` block with a value. |
//!
//! ## State Attributes
//! Attributes can be placed above a state declaration to modify its behavior.
//!
//! | Attribute | Description |
//! |---|---|
//! | `isolate` | Removes the state from implicit scheduling. Only reachable via `=> @state`. |
//! | `max_iter = N` | Caps the fixed-point loop to N iterations, then advances normally. |
//! | `max_iter = N => @state` | Same, but transitions to `@state` on exhaustion instead of advancing. |
//! | `max_entry = N` | Limits how many times this state can be entered. Returns on the (N+1)th entry. |
//! | `trace` | Emits diagnostics via [`log::trace!`] on state entry and rule evaluation. Requires a `log`-compatible backend. |
//!
//! ## Tracing
//! The `trace` attribute emits diagnostics through the [`log`] facade. Add a backend such as
//! [`env_logger`](https://docs.rs/env_logger) to capture the output:
//!
//! ```toml
//! [dependencies]
//! env_logger = "0.11.9"
//! ```
//!
//! ```rust,ignore
//! fn main() {
//!     env_logger::init();
//!     // ...
//! }
//! ```
//!
//! Then run with `RUST_LOG=trace` to capture output:
//!
//! ```text
//! # bash / zsh
//! RUST_LOG=trace cargo run -q 2> trace.log
//!
//! # PowerShell
//! $env:RUST_LOG="trace"; cargo run -q 2> trace.log
//! ```
//!
//! ## Example
//! A traffic light that cycles through red, green, and yellow twice before exiting.
//!
//! ```rust
//! use banish::banish;
//!
//! fn main() {
//!     let mut ticks: i32 = 0;
//!     banish! {
//!         // Returns on the third entry immediately
//!         #[max_entry = 2]
//!         @red
//!             announce ? {
//!                 ticks = 0;
//!                 println!("Red light");
//!             }
//!             timer ? ticks < 3 {
//!                 ticks += 1;
//!             }
//!
//!         @green
//!             announce ? {
//!                 println!("Green light");
//!             }
//!             timer ? ticks < 6 {
//!                 ticks += 1;
//!             }
//!
//!         @yellow
//!             announce ? {
//!                 println!("Yellow light");
//!             }
//!             timer ? ticks < 10 {
//!                 ticks += 1;
//!             } !? { => @red; }
//!     }
//! }
//! ```
//!
//! ## More Examples
//! See the [examples documentation](https://github.com/LoganFlaherty/banish/blob/main/docs/README.md)
//! for more examples.

pub use banish_derive::banish;
pub use log;