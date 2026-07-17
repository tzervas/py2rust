# Legacy Python package (deprecated)

The **primary** py2rust implementation is the Rust workspace:

```bash
cargo run -p py2rust -- analyze file.py
cargo run -p py2rust -- transpile file.py
```

This `src/py2rust/` tree is retained temporarily as a thin historical scaffold.
Do **not** expand it as the product path. Prefer `crates/py2rust` + `crates/py2rust-core`.
