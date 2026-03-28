---
name: banish
description: Reference for writing Rust code using the banish state machine framework. Use when writing, reviewing, or debugging banish! blocks.
user-invocable: false
---

# Banish Framework Reference

Banish is a declarative Rust library for rule-based state machines. You declare states
and rules; the framework generates the fixed-point scheduler, interaction tracking, and
state advancement at compile time.

```toml
[dependencies]
banish = "1.3.1"
```

---

## Core Syntax at a Glance

```rust
use banish::banish;

banish! {
    // Block attributes (optional, must come first)
    #![async]
    #![id = "my_machine"]
    #![dispatch(entry_state)]
    #![trace]

    // Block-level variables (live entire machine lifetime)
    let mut count: u32 = 0;

    // State declaration
    #[max_entry = 3 => @bail]
    @my_state
        // State-level variables (reset on every entry)
        let mut idx: usize = 0;

        // Conditional rule
        name ? condition { body }

        // Conditional rule with fallback (fallback does NOT re-evaluate)
        name ? condition { body } !? { else_body }

        // Conditionless rule (fires exactly once per entry, on first pass)
        name? { body }

        // Guarded transition (inside a rule body)
        => @other_state if condition;

        // Unconditional transition (inside a rule body)
        => @other_state;
}
```

---

## Execution Model

Within each state, rules evaluate **top to bottom** repeatedly until a full pass fires
nothing (fixed point). Then the scheduler advances to the next non-isolated state.

```
enter state
  └─ evaluate rules top-to-bottom
       ├─ any rule fired? → re-evaluate from top
       └─ no rules fired  → fixed point → advance to next state
```

- A rule "fires" when its condition is `true`. Firing sets `__interaction = true`.
- `!?` fallback branches do **not** set `__interaction` — they never cause re-evaluation.
- Conditionless rules (`name? { }`) fire once per entry (first pass only).
- Rule order matters: earlier rules can change state that affects later rules.

---

## Variables

| Scope | Where declared | Lifetime | Resets on re-entry? |
|---|---|---|---|
| Block-level | Before first `@state` | Entire machine | No |
| State-level | After `@state`, before first rule | One state entry | Yes — every entry |

---

## Transitions

```rust
// Implicit — scheduler advances to next non-isolated state after fixed point
// (no code needed)

// Explicit — jump immediately, bypasses scheduler
=> @target_state;

// Guarded — conditional jump; execution continues in rule body if false
=> @target_state if some_condition;
```

Transitions inside a rule body act immediately. They do not need to be the last
statement — code after a taken guarded transition is unreachable.

---

## Return Values

`return expr;` exits the entire `banish!` block with a value. The block is an
expression and can be assigned or returned from a function.

```rust
let result: String = banish! {
    @work
        done ? finished { return compute_result(); }
};
```

`break` exits the current state and lets the scheduler advance normally.
`continue` restarts rule evaluation from the top of the current state immediately.

---

## State Attributes

```rust
#[isolate]                          // removed from scheduler; explicit transition only
#[max_iter = N]                     // cap fixed-point loop; advance normally on exhaustion
#[max_iter = N => @state]           // cap fixed-point loop; transition on exhaustion
#[max_entry = N]                    // return on (N+1)th entry
#[max_entry = N => @state]          // transition on (N+1)th entry
#[trace]                            // emit log::trace! diagnostics (needs log backend)
```

Multiple attributes are comma-separated on one line:

```rust
#[isolate, max_iter = 1 => @finish]
@handler
    ...
```

---

## Isolated States

Isolated states are **invisible to the scheduler**. They are only entered via an
explicit `=> @state` transition. They must have a defined exit path (transition or
return), or use `max_iter` with a redirect.

```rust
#[isolate]
@error
    handle? { return Err("failed"); }
```

---

## Async

```rust
banish! {
    #![async]

    @fetch
        load? { let data = some_future().await; }
}
// The banish! block is now an async expression — must be .awaited
```

Or use the function attribute to avoid manual wiring:

```rust
#[banish::machine]   // must be outermost — before #[tokio::main]
#[tokio::main]
async fn main() {
    banish! { ... }  // async + .await injected automatically; id = "main"
}
```

---

## Dispatch (Resumable Machines)

```rust
use banish::BanishDispatch;

#[derive(BanishDispatch)]
enum Stage { Validate, Process, Finalize }

fn run(order: Order, resume: Stage) -> Result {
    banish! {
        #![dispatch(resume)]   // entry state determined at runtime

        @validate   ...
        @process    ...
        @finalize   done? { return Ok(()); }

        #[isolate]
        @rejected   handle? { return Err("bad"); }
    }
}
```

`BanishDispatch` maps `PascalCase` variant names → `snake_case` state names.
`Stage::Validate` → `"validate"` → `@validate`. Passing a variant with no matching
state **panics at runtime**.

---

## Tracing

```toml
banish = { version = "1.3.1", features = ["trace-logger"] }
```

```rust
fn main() {
    banish::init_trace(None);           // stderr
    banish::init_trace(Some("t.log"));  // file
    ...
}
```

Or bring your own `log`-compatible backend. Diagnostics are emitted as `log::trace!`.

---

## Patterns & Gotchas

**Cycling between states** — use `max_iter` with a redirect instead of an explicit
loop, to avoid unbounded recursion:

```rust
#[max_iter = 1 => @b]
@a  attack? { ... }

#[max_iter = 1 => @a]
@b  attack? { ... }
```

**One-shot setup per state entry** — conditionless rules run once, then never again
until the state is re-entered:

```rust
@load
    init? { data = load_data(); }   // fires once
    process ? data.is_some() { ... }
```

**Fallback as else-branch that needs a loop** — fallbacks do not re-evaluate; use an
explicit transition if re-evaluation is needed after the else branch:

```rust
valid ? x > 0 { use(x); } !? { x = default; => @reset; }
```

**Rule order is evaluation order** — if rule A mutates a value rule B checks, put A
before B to make B see the updated value in the same pass.

**State-level `let` resets every entry** — don't store cross-entry state there; use
block-level variables instead.

**`#[banish::machine]` must be the outermost attribute** — it runs first and transforms
the function before any runtime macro (e.g. `#[tokio::main]`) sees it.
