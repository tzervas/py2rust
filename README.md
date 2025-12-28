# py2rust

Python to Rust transpiler and code conversion assistant.

## Installation

```bash
pip install py2rust
```

## Usage

Transpile Python code to Rust:

```bash
py2rust transpile my_script.py --output my_script.rs
```

Analyze Python code for Rust conversion compatibility:

```bash
py2rust analyze my_script.py
```

## Features

- **AST-based Transpilation**: Convert Python syntax to equivalent Rust constructs
- **Compatibility Analysis**: Identify code patterns that need manual conversion
- **Incremental Conversion**: Start with automated conversion and refine manually
- **Type Inference**: Basic type inference for Python variables

## Current Limitations

This is an early-stage transpiler that handles basic function conversion. Complex Python features like:

- Classes and inheritance
- Exception handling
- Dynamic typing
- Metaprogramming

Require manual conversion and are flagged during analysis.

## Development

This project uses [uv](https://github.com/astral-sh/uv) for dependency management.

```bash
# Install dependencies
uv sync

# Run tests
uv run pytest

# Format code
uv run black src/
uv run isort src/
```

## License

MIT License - see [LICENSE](LICENSE) for details.