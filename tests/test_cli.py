"""Tests for py2rust CLI."""

import pytest
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