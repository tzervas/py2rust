"""Tests for py2rust CLI."""

from click.testing import CliRunner

from py2rust.cli import main


def test_cli_help():
    """Test that CLI shows help."""
    runner = CliRunner()
    result = runner.invoke(main, ["--help"])
    assert result.exit_code == 0
    assert "Python to Rust transpiler" in result.output


def test_analyze_command():
    """Test analyze command with basic Python code."""
    runner = CliRunner()
    with runner.isolated_filesystem():
        # Create a simple Python file
        with open("test.py", "w") as f:
            f.write("""
def hello():
    print("Hello, World!")

class MyClass:
    pass
""")

        result = runner.invoke(main, ["analyze", "test.py"])
        assert result.exit_code == 0
        assert "Compatibility issues found" in result.output
        assert "Class" in result.output


def test_transpile_with_type_annotations():
    """Test transpile command with type annotations mapped to Rust types."""
    runner = CliRunner()
    with runner.isolated_filesystem():
        with open("test.py", "w") as f:
            f.write("""
def add(x: int, y: float) -> float:
    pass

def greet(name: str) -> None:
    pass

def check(flag: bool) -> bool:
    pass
""")

        result = runner.invoke(main, ["transpile", "test.py", "--output", "test.rs"])
        assert result.exit_code == 0
        assert "Transpilation completed successfully" in result.output

        with open("test.rs") as f:
            rust_content = f.read()

        assert "fn add(x: i32, y: f64) -> f64 {" in rust_content
        assert "fn greet(name: &str) {" in rust_content
        assert "fn check(flag: bool) -> bool {" in rust_content
        assert "0.0  // Placeholder return" in rust_content
        assert "false  // Placeholder return" in rust_content


def test_transpile_without_annotations():
    """Test transpile command defaults to i32 for unannotated functions."""
    runner = CliRunner()
    with runner.isolated_filesystem():
        with open("test.py", "w") as f:
            f.write("""
def untyped_func(a, b):
    pass
""")

        result = runner.invoke(main, ["transpile", "test.py", "--output", "test.rs"])
        assert result.exit_code == 0
        assert "Transpilation completed successfully" in result.output

        with open("test.rs") as f:
            rust_content = f.read()

        assert "fn untyped_func(a: i32, b: i32) -> i32 {" in rust_content
        assert "0  // Placeholder return" in rust_content


def test_transpile_invalid_syntax():
    """Test transpile command reports Python syntax errors gracefully."""
    runner = CliRunner()
    with runner.isolated_filesystem():
        with open("test.py", "w") as f:
            f.write("""
def invalid_syntax(
""")

        result = runner.invoke(main, ["transpile", "test.py"])
        assert result.exit_code != 0
        assert "Python syntax error" in result.output


def test_analyze_invalid_syntax():
    """Test analyze command reports Python syntax errors gracefully."""
    runner = CliRunner()
    with runner.isolated_filesystem():
        with open("test.py", "w") as f:
            f.write("""
def invalid_syntax(
""")

        result = runner.invoke(main, ["analyze", "test.py"])
        assert result.exit_code != 0
        assert "Python syntax error" in result.output
