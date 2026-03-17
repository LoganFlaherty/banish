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
7. [Examples](#examples)
8. [Error Reference](#error-reference)
9. [Known Limitations](#known-limitations)

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
        done? { return; }

    @second
        // never reached in this example
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
        println!("initialized");
    }
    // remaining rules run after init fires once
```

### Fallback Branches

```
name ? condition { body } !? { else_body }
```

A fallback branch runs when the rule's condition is `false`. Unlike the rule body, a fallback branch does **not** set `__interaction = true`. It does not trigger a re-evaluation pass on its own. If you need to loop after a fallback, use an explicit `=> @state` transition.

```rust
@check
    valid ? value > 0 {
        println!("valid: {}", value);
    } !? {
        println!("invalid, resetting");
        value = default;
        => @check; // explicit re-entry needed here
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
    timer ? ticks < 10 { ticks += 1; }
    !? {
        ticks = 0;
        => @red;  // jump back to @red directly
    }
```

Transition targets must refer to a declared state name. Unknown targets are a compile error at the `=> @state` callsite.

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
    @evaluate
        pass ? score >= 60 { return "pass"; }
        fail ? score <  60 { return "fail"; }
};
```

```rust
fn find(buffer: &[String], target: &str) -> Option<usize> {
    let mut idx = 0;
    banish! {
        @search
            not_found ? idx >= buffer.len() { return None; }
            found ? buffer[idx] == target { return Some(idx); }
            advance ? idx < buffer.len() { idx += 1; }
    }
}
```

The return type is inferred by the Rust compiler from the `return` expressions in the block. If the block returns nothing, it evaluates to `()`.

---

## State Attributes

Attributes are declared above a state and modify its runtime behavior. Multiple attributes are comma-separated.

```rust
#[isolate, max_iter = 10 => @fallback, trace]
@my_state
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
        }
        done? { return; }

    #[isolate, max_iter = 1 => @main]
    @handler
        handle? {
            println!("handling");
        }
}
```

**Constraints:**
* An isolated state must have a defined exit: either a `return`, `=> @state` in its rules, or `max_iter = N => @state`. Isolated states with no exit are a compile error. `max_entry = N => @state` does not satisfy this requirement because it only fires on the (N+1)th entry and provides no exit for entries 1 through N.

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
    // ...
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

## Examples

### Hello World

A minimal single-state machine.

```rust
use banish::banish;

fn main() {
    banish! {
        @hello
            print? {
                println!("Hello, world!");
                return;
            }
    }
}
```

---

### Traffic Lights

Demonstrates implicit state advancement, conditionless rules, fallback transitions, and incorporating attributes.

```rust
use banish::banish;

fn main() {
    let mut ticks: i32 = 0;
    banish! {
        #[max_entry = 2]
        @red
            announce? {
                ticks = 0;
                println!("Red light");
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

`@red` and `@green` each reach their fixed point once their timer rule stops firing. `@yellow`'s fallback branch transitions back to `@red` explicitly. Without this, fallback branches do not trigger re-evaluation.

---

### Dragon Fight

Demonstrates early return with a value, external crate usage, multi-state transitions, and fallback branches.

```rust
use banish::banish;
use rand::prelude::*;

fn main() {
    let mut rng = rand::rng();
    let mut player_hp = 20;
    let mut dragon_hp = 50;

    println!("BATTLE START");

    let result: &str = banish! {
        @player_turn
            attack? {
                let damage = rng.random_range(5..15);
                dragon_hp -= damage;
                println!("You hit the dragon for {} dmg! (Dragon HP: {})", damage, dragon_hp);
            }
            check_win ? dragon_hp <= 0 { return "Victory!"; }
            end_turn? { => @dragon_turn; }

        @dragon_turn
            attack? {
                let damage = rng.random_range(2..20);
                player_hp -= damage;
                println!("Dragon breathes fire for {} dmg! (Player HP: {})", damage, player_hp);
            }
            halfway ? player_hp <= 10 && dragon_hp <= 25 {
                println!("\nThe battle is getting intense!\n");
            } !? {
                println!("\nThe dragon is getting weak!\n");
            }
            check_loss ? player_hp <= 0 { return "Defeat..."; }
            end_turn? { => @player_turn; }
    };

    println!("GAME OVER: {}", result);
}
```

`end_turn?` is conditionless, so it fires exactly once per state entry, after `attack?` has fired and `check_win`/`check_loss` have been evaluated. This ensures the turn always ends rather than looping indefinitely.

---

### Find Index

Demonstrates `banish!` inside a regular function, returning through the enclosing function's return type.

```rust
use banish::banish;

fn find_index(buffer: &[String], target: &str) -> Option<usize> {
    let mut idx = 0;
    banish! {
        @search
            // bounds check must come first to prevent out-of-bounds indexing below
            not_found ? idx >= buffer.len() { return None; }
            found ? buffer[idx] == target { return Some(idx); }
            advance ? idx < buffer.len() { idx += 1; }
    }
}
```

Rule ordering is significant here. `not_found` must precede `found` and `advance`. If `idx` is out of bounds, `buffer[idx]` would panic.

---

### Double For Loop

Demonstrates self-transitions and returning a tuple value.

```rust
use banish::banish;

fn main() {
    let mut x = 0;
    let mut y = 0;
    let mut cnt = 0;

    // Equivalent to:
    // for y in 0..10 {
    //     for x in 0..10 { cnt += 1; }
    // }
    let (x, y, cnt) = banish! {
        @for_loops
            for_x ? x != 10 {
                x += 1;
                cnt += 1;
            } !? {
                x = 0;
                y += 1;
                if y == 10 { return (x, y, cnt); }
                => @for_loops;
            }
    };
}
```

The fallback branch fires when `x == 10`. Fallback branches do not trigger re-evaluation, so `=> @for_loops` is required to restart the state.

---

## Error Reference

Banish validates the macro input at compile time and produces span-accurate errors pointing at the offending token.

| Error | Cause | Fix |
|---|---|---|
| ``Duplicate state name `X` `` | Two states share the same name. | Rename one of the states. |
| ``Duplicate rule name `X` in state `Y` `` | Two rules in the same state share a name. | Rename one of the rules. |
| ``Unknown state transition `X` `` | A `=> @state` transition, `max_iter` redirect, or `max_entry` redirect refers to an undeclared state. | Declare the state or fix the name. |
| ``Final state `X` must have a return or state transition statement`` | The last non-isolated state has no exit path. | Add `return` or `=> @state` to at least one rule. |
| ``Isolated state `X` has no exit...`` | An isolated state has no `return`, `=> @state`, or `max_iter = N => @state`. | Add an exit path. `max_entry = N => @state` alone is not sufficient. |
| ``Unknown state attribute `X` `` | An unrecognised attribute was used inside `#[...]`. | Check spelling against the supported attribute list. |
| ``Duplicate attribute `X` `` | The same attribute appears more than once in `#[...]`. | Remove the duplicate. |
| `` `max_iter` value must be greater than zero `` | `max_iter = 0` was specified. | Use a value of at least 1. |
| `` `max_entry` value must be greater than zero `` | `max_entry = 0` was specified. | Use a value of at least 1. |
| ``A state may only have one attribute block `#[...]` `` | A state has two or more `#[...]` blocks. | Merge all attributes into a single comma-separated `#[...]` block. |
| ``Rule `X` cannot have an `!?` branch without a condition`` | A conditionless rule has a fallback branch. | Either add a condition to the rule or remove the `!?` block. |

---

## Known Limitations

**Transitions are statement-level only.** `=> @state` must appear as a standalone statement inside a rule body or fallback branch. It cannot be used inside a nested block, closure, or `if` expression within a rule body. Use a top-level conditional rule instead.

```rust
// Does not work
step? { if condition { => @other; } }

// Works
step ? condition { => @other; }
```

**`banish!` cannot be used in `const` contexts.** The generated code uses mutable variables and loops, which are not const-evaluable.

**Rule names are identifiers, not strings.** They exist for readability and error messages only. They cannot be inspected or matched at runtime.