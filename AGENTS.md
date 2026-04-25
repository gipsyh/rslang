# Project Guide For Coding Agents

This repository contains `rslang`, a Rust library that calls system `slang` and
lowers slang AST JSON into a smaller Rust AST for future semantic analysis.

## High-Level Intent

Do not treat this crate as a C++ binding to slang. The boundary is the `slang`
CLI and its JSON output:

```text
SystemVerilog files -> slang --ast-json -> serde_json::Value -> rslang AST
```

The central design goal is to make later analysis passes work on stable Rust
types instead of ad hoc JSON traversal.

## Important Files

- `src/lib.rs`: public exports and convenience `parse_file` / `parse_files`.
- `src/slang.rs`: command wrapper for system `slang`.
- `src/ast/`: stable Rust AST data structures used by analysis, with per-node
  lowering helpers from slang JSON.
- `src/error.rs`: crate error type.
- `tests/multiplier.rs`: detailed example of expected lowered structure.
- `tests/fvbench.rs`: coverage over all bundled fvbench cases.

## Editing Guidelines

- Keep `src/ast/` analysis-oriented. Add fields when they serve semantic
  analysis, not just to mirror every slang JSON field.
- Keep unsupported syntax as `Unknown` until there is a clear semantic use for
  a richer node.
- Prefer adding lowering support in small, test-backed increments.
- Do not silently accept failed `slang` commands in library code. Tests may pass
  explicit slang flags such as `--disable-analysis` when they intentionally
  focus on lowering rather than diagnostics.
- Keep source locations available on new AST nodes when slang provides them.

## Common Tasks

### Add Support For A New Expression

1. Add an `Expr` variant in `src/ast/expr.rs`.
2. Extend `lower_expr` in `src/ast/expr.rs`.
3. Add or update a test that exercises the new slang JSON kind.
4. Run `cargo test` and `cargo clippy --all-targets -- -D warnings`.

### Add Support For A New Statement

1. Add a `Stmt` variant in `src/ast/statement.rs`.
2. Extend `lower_stmt` in `src/ast/statement.rs`.
3. Ensure `Stmt::source` and `Stmt::for_each` handle the new variant.
4. Add or update a test.

### Add A New fvbench Case

1. Place the `.sv` file under `tests/fvbench/<case>/`.
2. Add a case entry to `tests/fvbench.rs`.
3. Use `.arg("--top").arg("<top>")`.
4. If slang post-analysis rejects the design but AST lowering is still the goal,
   add `.arg("--disable-analysis")` in the test path, not in production defaults.

## Verification Commands

```bash
cargo fmt
cargo test
```

## Known Shape Of The Current AST

- `Design` contains top-level modules lowered from root `Instance` members.
- `Module` stores parameters, ports, nets, variables, and procedural blocks.
- `Stmt` supports empty, block, timed, if, assignment, expression, assertion,
  and unknown statements.
- `Expr` supports named values, integer literals, unary and binary operators,
  conversions, assignment expressions, and unknown expressions.
