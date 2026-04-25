# rslang

`rslang` is a small Rust library for turning SystemVerilog designs parsed by
[slang](https://github.com/MikePopoloski/slang) into Rust data structures that
are easier to use in later semantic analysis passes.

The library does not bind to slang's C++ ABI. Instead, it calls the system
`slang` executable, asks it for AST JSON, and lowers that JSON into a compact
Rust IR.

## What This Crate Does

- Runs system `slang` with `--ast-json`.
- Parses the JSON output with `serde_json`.
- Lowers slang's elaborated AST into Rust types such as `Design`, `Module`,
  `Port`, `Signal`, `ProcedureBlock`, `Stmt`, and `Expr`.
- Keeps source locations when `--ast-json-source-info` is enabled.
- Preserves unsupported nodes as `Unknown` variants so analysis can continue
  while the IR grows.

## Requirements

- Rust 2024 edition.
- A working `slang` executable in `PATH`.

You can check the frontend manually:

```bash
slang --version
```

## Quick Start

```rust
use rslang::{parse_file, ProcedureKind};

fn main() -> rslang::Result<()> {
    let design = parse_file("tests/fvbench/multiplier/multiplier.sv")?;
    let module = design.module("multiplier").expect("top module");

    println!("module: {}", module.name);
    println!("ports: {}", module.ports.len());

    for procedure in &module.procedures {
        if procedure.kind == ProcedureKind::Always {
            println!("found always block");
        }
    }

    Ok(())
}
```

For designs that need explicit slang options, use `Slang` directly:

```rust
use rslang::Slang;

let design = Slang::default()
    .arg("--top")
    .arg("arbiter")
    .arg("--disable-analysis")
    .parse_file("tests/fvbench/arbiter/arbiter.sv")?;
```

`--disable-analysis` is useful for tests that only care about AST/IR lowering
when slang's later analysis passes reject an assertion or property.

## Public API Map

- `parse_file(path)` and `parse_files(paths)` are convenience functions.
- `Slang` wraps the system `slang` command.
- `Slang::arg` and `Slang::args` pass extra frontend options such as `--top`,
  `--std`, or `--disable-analysis`.
- `Slang::ast_json_for_file` returns raw slang AST JSON as `serde_json::Value`.
- `lower_slang_ast` and `lower_slang_ast_str` lower raw AST JSON into `Design`.

## IR Overview

The IR is intentionally small and analysis-oriented:

- `Design` contains top-level lowered modules.
- `Module` contains parameters, ports, nets, variables, and procedural blocks.
- `SignalKind::Net` represents net-like objects such as wires and port internals.
- `SignalKind::Variable` represents storage-like objects such as `reg` and
  procedural variables.
- `Stmt` covers blocks, timed statements, conditionals, assignments, assertions,
  expressions, and unknown statements.
- `Expr` covers named values, integer literals, unary and binary operations,
  conversions, assignments, and unknown expressions.

The `Unknown` variants are deliberate. They let semantic analysis start on the
parts that are already understood while preserving enough information to add
more lowering support later.

## How The Pipeline Works

1. `src/slang.rs` builds a command like:

   ```text
   slang --quiet --ast-json - --ast-json-source-info <extra args> <files>
   ```

2. stdout is parsed as JSON.
3. `src/lower.rs` walks the JSON and constructs the Rust IR from `src/ir.rs`.
4. Errors are reported through `src/error.rs`.

## Tests

Run all tests:

```bash
cargo test
```

Run static checks:

```bash
cargo clippy --all-targets -- -D warnings
```

The test suite includes:

- `tests/multiplier.rs`: detailed structural checks for the multiplier case.
- `tests/fvbench.rs`: one smoke/shape test for each `tests/fvbench` case.

## Current Limitations

- This is not a full SystemVerilog IR yet.
- The lowering currently focuses on the constructs needed by the included
  examples.
- Some slang AST nodes are represented as `Unknown`.
- Nested instances, concurrent assertions, memories, selects, calls, and richer
  property expressions may need more dedicated IR as analysis requirements grow.
- The library expects system `slang` to be installed rather than vendoring it.

## Suggested Reading Order

For future maintainers or coding agents, read files in this order:

1. `README.md` for the intent and pipeline.
2. `src/lib.rs` for the public API.
3. `src/ir.rs` for the data model.
4. `src/slang.rs` for frontend invocation.
5. `src/lower.rs` for AST JSON lowering.
6. `tests/multiplier.rs` and `tests/fvbench.rs` for expected behavior.

