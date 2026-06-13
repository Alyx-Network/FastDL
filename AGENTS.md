# Rule priority

1. Structural rules override naming rules.
2. Naming rules override style rules.
3. Database rules override optimization rules.
4. Logging rules override minimalism rules.
5. If two rules conflict, follow the more restrictive rule.

***

# Workflow

- Build only with `cargo build`. Never use `cargo run` to validate.
- Use parallel tools whenever applicable.
- Execute automatically when safe.
- Do not ask for confirmation unless blocked by missing data, safety, or irreversibility.
- Do not refactor architecture unless explicitly requested.
- Do not move files unless explicitly requested.
- Modify only what is asked.

***

# Runtime boundaries

## Server / backend files

All `.rs` files are server files.

Includes: HTTP handlers, database logic, payment processing, billing operations, filesystem handling, validation logic, entity comparison, configuration management.

- Use `snake_case` for all identifiers.
- Do not include UI rendering logic.
- Do not import frontend or browser-specific crates.
- Do not access browser-specific APIs.
- Wrap all potentially unsafe operations in `Result`-returning functions; propagate errors with `?`.
- All logging must use the structured logging system (`tracing` crate).

***

## Logic categories

Factor out common logic into central modules; entity-specific files build upon these shared bases.

A main module (for example, `query.rs`) exports general-purpose utilities while domain files (for example, `user.rs`, `plan.rs`) import and extend them. Mixing related responsibilities within a file is allowed when it increases modularity and reduces redundancy.

- Good: `query.rs` contains generic query helpers; `user.rs` and `plan.rs` import and extend them.
- Good: `entity.rs` combines all CRUD logic for a domain to reduce duplication.
- Good: `access.rs` provides base config helpers, imported by domain files.
- Bad: Repeating nearly identical logic across domain files when a shared module would suffice.

***

# File classification

## Configuration files

- Only export constants or static configuration structs.
- Do not define functions, perform logic, or validate data.

## Access files

- Retrieve and return config or static data.
- Must not perform validation, comparison, or produce side effects.

## Comparison files

- Export pure functions only.
- Must not interact with databases, log, mutate state, or cause side effects.

## Validation files

- Enforce business logic using comparison functions where applicable.
- Must not perform database queries, mutate state, or access the filesystem.

## Query files

- Read-only database operations.
- Always specify exact columns in `SELECT` statements.
- Must not write, mutate, or validate data.

## Write files

- Write operations for the database only.
- Always wrap writes in `Result`; return structured errors.
- Must log failures with structured context via `tracing`.
- Never expose raw error objects or sensitive data.

***

# Naming

## General rules

- Variable names must be descriptive and at least 4 characters. Conventional loop counters `i`, `j`, `k` are permitted.
- No clarity-reducing abbreviations.
- Prefer single-word names.
- Inline variables if used once.
- Avoid unnecessary destructuring.
- Never mix naming conventions within a file.

## Naming matrix

| Context                      | Convention |
| ---------------------------- | ---------- |
| Database                     | snake_case |
| ORM model fields             | snake_case |
| Server logic                 | snake_case |
| HTTP handlers                | snake_case |
| JSON fields                  | snake_case |
| Payment fields               | snake_case |
| Types / structs / enums      | PascalCase |
| Traits                       | PascalCase |

***

# Code style

- Prefer `let` bindings that are not `mut`; use `mut` only when mutation is required.
- Avoid variable reassignment.
- Eliminate `else` blocks in favor of early returns.
- Never use untyped generics or `dyn Any` without justification; prefer explicit types and trait bounds.
- Use iterator adapters (`map`, `filter`, `fold`) instead of explicit loops where it improves clarity.
- Inline logic and variables used only once.
- Group related logic in a single function; extract only when reused in multiple locations.
- Do not include comments in implementation files.
- Implement solutions minimally and directly.
- Only introduce abstractions when identical logic is reused at least twice.
- Never create catch-all or generic utility files.
- Prefer explicitness for critical logic; avoid implicit or ambiguous patterns.

***

# Control flow

- Prefer early returns over nested conditionals.
- Never use implicit truthiness for critical control flow.
- Avoid deeply nested branches; flatten logic for readability.
- Use `match` when branching on multiple values of a single variable.
- Always handle all branches, including `_` or fallback arms.

Good:

```rust
fn check_status(status: &str) -> &str {
    if status == "success" { return "ok"; }
    if status == "pending" { return "waiting"; }
    if status == "failed" { return "error"; }
    "unknown"
}
```

Good (match):

```rust
fn check_status(status: &str) -> &str {
    match status {
        "success" => "ok",
        "pending" => "waiting",
        "failed"  => "error",
        _         => "unknown",
    }
}
```

Bad:

```rust
fn check_status(status: &str) -> &str {
    if status == "success" { return "ok"; }
    else if status == "pending" { return "waiting"; }
    else if status == "failed" { return "error"; }
    else { "unknown" }
}
```

Bad (implicit truthiness via `Option`):

```rust
fn handle(value: Option<u32>) -> &'static str {
    if value.is_some() { "exists" } else { "missing" }
}
```

Good (explicit pattern match):

```rust
fn handle(value: Option<u32>) -> &'static str {
    match value {
        Some(_) => "exists",
        None    => "missing",
    }
}
```

***

# Variables

- Prefer immutable `let`; use `let mut` only when mutation is necessary.
- Favor immediate assignment and initialization.
- Use ternary-style expressions or `match` for straightforward value selection; do not initialize with a placeholder then reassign.

Good:

```rust
let user_role = if is_admin { "admin" } else { "user" };
```

Bad:

```rust
let mut user_role;
if is_admin { user_role = "admin"; }
else { user_role = "user"; }
```

Edge (aggregation):

```rust
let total: f64 = items.iter().map(|item| item.price).sum();
```

***

# Destructuring

- Destructure only if accessing the same field repeatedly in a local scope.
- Do not destructure multiple fields used only once.
- For function arguments, destructure only if all or most fields are used.

Good:

```rust
user.id;
user.profile.name;
```

Good (reused field):

```rust
let name = &user.profile.name;
```

Bad:

```rust
let UserProfile { name, age } = user.profile;
```

***

# Error handling

- All fallible operations must return `Result`; use `?` to propagate.
- Always include the operation name, relevant identifiers, and metadata when logging errors.
- Never ignore errors or allow silent failures.
- Never expose sensitive data in error messages or logs.
- Never return or log raw database errors to callers.
- Use `tracing::error!` with structured fields for all server-side error logging.
- Always log operation name, identifiers, and event location.

***

# Logging

- Use `tracing` as the logging utility for all server-side code.
- Always include operation name, identifiers, and event location.
- Never log only raw error messages; always supplement with descriptive text and structured fields.
- Standardize log message format:
  ```
  "Failed to create session [create_session_failed]"
  ```
- Use appropriate macros: `tracing::info!`, `tracing::warn!`, `tracing::error!`, `tracing::debug!`.

***

# Database (SQLx / SeaORM)

- Use the project's chosen ORM or query builder exclusively for all database operations.
- All schema fields and model names must use `snake_case`.
- Do not remap column names unless technically required.
- Keep schemas minimal and normalized.
- Only introduce relations where necessary.
- Add explicit indexes to fields that are frequently queried, filtered, or joined.
- Use database transactions for multi-step or dependent operations that must be atomic.
- Always wrap mutations in `Result`-returning functions.
- Never return or log raw database errors; sanitize and provide contextual details.
- When logging database errors, include operation name, model(s), and relevant identifiers.
- Import the shared database pool from its central location. Search the codebase to confirm the correct path.

## Query rules

- Write queries inline unless reused; extract to a shared function only when reused.
- Always select exact columns needed; never `SELECT *`.
- Never fetch more fields than required; never expose confidential fields.
- Use a single well-constructed query instead of multiple smaller queries for complex lookups.
- Use `LIMIT`, `OFFSET`, and `WHERE` for pagination and filtering.

## Write and mutation rules

- Always perform mutations inside `Result`-returning functions and handle errors with full context.
- Use transactions for multiple dependent mutations.
- Log all errors with structured context: operation, identifiers, input data, and error details.
- Always return simplified, sanitized responses; never include sensitive data or raw ORM objects.
- Do not expose internal database state, raw errors, traces, or stack information to external consumers.
- Audit mutations for potential side effects such as cascading deletes.
- Organize mutation helpers by entity or functional domain; use central mutation modules when patterns repeat.

***

# Plan hierarchy

- Tier is the sole determinant of plan hierarchy, upgrade, and downgrade direction.
- Pricing, cost, features, and limits cannot be used to infer plan order or migration path.
- Cost difference is presented to users as information only; never used for hierarchy logic.
- All plan transition logic must pull from a normalized tier definition in a single authoritative source (config or access file).
- Never implement conditional logic that checks prices to decide upgrade or downgrade direction.

***

# Utility and data placement

- Shared logic (used 2+ times) must live in a `lib` module or shared crate; single-use logic stays inline.
- Configuration logic must reside in the corresponding config file or module.
- Comparison logic must be in dedicated comparison files.
- Business rules must be in validation files.
- Never create generic or catch-all utility files; utilities must be domain-specific and justified by repeated usage.

***

# Server actions and client data

- All data exposed to external consumers must originate from validated server-side logic.
- Every mutation, permission check, or sensitive calculation must be enforced on the server.
- All inputs received from external sources must be re-validated on the server before use or storage.
- Never expose API secrets, business logic, or configuration in responses.
- All cross-tier, plan, or permission changes must be validated and confirmed by a server-side handler before being applied.

***

# Optimization

- Lazy-initialize external resources where possible.
- Loaded resources override defaults.
- Avoid unnecessary abstractions.
- Avoid recursive loading unless required.
- Group related operations.
- Prefer direct implementation.
- Show only diffs when editing.
- Maximize reuse of existing utilities.

***

# Testing

- Avoid mocks; test real implementation.
- Do not duplicate logic in tests.
- Never run tests from workspace root; run from crate directories only (`cargo test` inside the relevant crate).

***

# Strictly forbidden

- Mixing business logic with HTTP handler setup.
- Comparing price to determine plan hierarchy.
- Silent error swallowing.
- Returning raw database errors.
- Multiple responsibilities per file.
- Implicit truthiness for critical validation.
- Refactoring architecture without explicit instruction.
- Renaming entities without request.
- Moving files without request.

***

# Decision tree

When adding logic:

1. Does it read configuration? → config or access.
2. Does it compare two entities? → comparison.
3. Does it enforce rules? → validation.
4. Does it read database? → query.
5. Does it write database? → write.
6. Does it mix categories? → split.

If unsure → split.