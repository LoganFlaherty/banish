//! # Banish
//! Banish is a declarative DSL for building rule-based state machines in Rust. 
//! States evaluate their rules until reaching a fixed point or triggering a transition, reducing control flow boilerplate.
//!
//! ## Syntax
//! - **@state** : Defines a state. A state re-evaluates until no rule triggers or a transition occurs.
//! - **rule ? condition {}** : Defines a rule. Executes if its condition is true. Rules execute from top to bottom.
//! - **!? {}** : Defines a fallback branch. Executes when the rule's condition is false.
//! - **rule ? {}** : A rule without a condition. Executes exactly once per state entry. Cannot have a fallback branch.
//! - **=> @state;** : Explicit transition. Immediately transfers to another state. Valid only at the top level of a rule body.
//! - **return value;** : Immediately exit banish! and return a value if provided.
//! - **break;** : Immediately exits out of the state.
//!
//! ## Examples
//! https://github.com/LoganFlaherty/banish/blob/main/docs/README.md
//!
//! ```rust
//! use banish::banish;
//!
//! fn main() {
//!     let mut ticks: i32 = 0;
//!     let mut loop_count: i32 = 0;
//!     banish! {
//!         @red
//!             announce ? {
//!                 ticks = 0;
//!                 println!("Red light");
//!                 loop_count += 1;
//!              }
//!
//!             timer ? ticks < 3 {
//!                 ticks += 1;
//!             }
//!
//!         @green
//!             announce ? {
//!                 println!("Green light");
//!             }
//!
//!             timer ? ticks < 6 {
//!                 ticks += 1;
//!             }
//!
//!         @yellow
//!             announce ? {
//!                 println!("Yellow light");
//!             }
//!
//!             timer ? ticks < 10 {
//!                 ticks += 1;
//!             } !? {
//!                 loop_count += 1;
//!                 => @red;
//!             }
//!
//!             end ? loop_count = 1 { return; }
//! }
//! ```

pub use banish_derive::banish;