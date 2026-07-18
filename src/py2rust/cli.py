"""Command-line interface for py2rust transpiler."""

import ast
import sys
from pathlib import Path

import click


@click.group()
@click.version_option()
def main():
    """Python to Rust transpiler and code conversion assistant."""
    pass


@main.command()
@click.argument("python_file", type=click.Path(exists=True))
@click.option("--output", "-o", type=click.Path(), help="Output Rust file")
@click.option("--module", "-m", help="Rust module name")
def transpile(python_file, output, module):
    """Transpile Python code to Rust."""
    python_path = Path(python_file)
    output_path = Path(output) if output else python_path.with_suffix(".rs")
    module_name = module or python_path.stem

    click.echo(f"🔄 Transpiling {python_path} to Rust")
    click.echo(f"📁 Output: {output_path}")
    click.echo(f"📦 Module: {module_name}")

    # Read and parse Python code
    with open(python_path, encoding="utf-8") as f:
        python_code = f.read()

    try:
        tree = ast.parse(python_code)
        transpiler = PythonToRustTranspiler(module_name)
        rust_code = transpiler.transpile(tree)

        # Write Rust code
        with open(output_path, "w", encoding="utf-8") as f:
            f.write(rust_code)

        click.echo("✅ Transpilation completed successfully")

    except SyntaxError as e:
        click.echo(f"❌ Python syntax error: {e}", err=True)
        sys.exit(1)
    except Exception as e:
        click.echo(f"❌ Transpilation failed: {e}", err=True)
        sys.exit(1)


@main.command()
@click.argument("python_file", type=click.Path(exists=True))
def analyze(python_file):
    """Analyze Python code for Rust conversion compatibility."""
    python_path = Path(python_file)

    click.echo(f"🔍 Analyzing {python_path} for Rust compatibility")

    try:
        with open(python_path, encoding="utf-8") as f:
            python_code = f.read()
    except Exception as e:
        click.echo(f"❌ Failed to read {python_path}: {e}", err=True)
        sys.exit(1)

    try:
        tree = ast.parse(python_code)
        analyzer = CompatibilityAnalyzer()
        issues = analyzer.analyze(tree)

        if issues:
            click.echo("⚠️  Compatibility issues found:")
            for issue in issues:
                click.echo(f"  - {issue}")
        else:
            click.echo("✅ Code appears compatible with Rust conversion")

    except SyntaxError as e:
        click.echo(f"❌ Python syntax error: {e}", err=True)
        sys.exit(1)
    except Exception as e:
        click.echo(f"❌ Analysis failed: {e}", err=True)
        sys.exit(1)


class PythonToRustTranspiler:
    """Transpiler for converting Python AST to Rust code."""

    def __init__(self, module_name: str):
        self.module_name = module_name
        self.indent_level = 0

    def transpile(self, tree: ast.AST) -> str:
        """Transpile Python AST to Rust code."""
        lines = [
            f"// Generated from Python module: {self.module_name}",
            "// This is an automated conversion - manual review required!",
            "",
        ]

        # Add basic Rust structure
        lines.extend(self._generate_module_structure(tree))

        return "\n".join(lines)

    def _generate_module_structure(self, tree: ast.AST) -> list[str]:
        """Generate basic Rust module structure."""
        lines = []

        # Add functions
        for node in ast.walk(tree):
            if isinstance(node, ast.FunctionDef):
                lines.extend(self._transpile_function(node))

        return lines

    def _map_type(self, node: ast.AST | None, default: str = "i32") -> str:
        """Map Python type AST node to Rust type representation."""
        if node is None:
            return default

        if isinstance(node, ast.Name):
            mapping = {
                "int": "i32",
                "float": "f64",
                "str": "&str",
                "bool": "bool",
            }
            return mapping.get(node.id, default)

        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            mapping = {
                "int": "i32",
                "float": "f64",
                "str": "&str",
                "bool": "bool",
            }
            return mapping.get(node.value, default)

        return default

    def _get_return_type(self, returns_node: ast.AST | None) -> str | None:
        """Get the return type string or None if it returns None / is omitted."""
        if returns_node is None:
            return "i32"

        if isinstance(returns_node, ast.Name):
            if returns_node.id == "None":
                return None
            mapping = {
                "int": "i32",
                "float": "f64",
                "str": "&str",
                "bool": "bool",
            }
            return mapping.get(returns_node.id, "i32")

        if isinstance(returns_node, ast.Constant):
            if returns_node.value is None or returns_node.value == "None":
                return None
            if isinstance(returns_node.value, str):
                mapping = {
                    "int": "i32",
                    "float": "f64",
                    "str": "&str",
                    "bool": "bool",
                }
                return mapping.get(returns_node.value, "i32")

        return "i32"

    def _transpile_function(self, func: ast.FunctionDef) -> list[str]:
        """Transpile a Python function to Rust."""
        lines = []

        # Function signature
        args_list = []
        for arg in func.args.args:
            arg_type = self._map_type(arg.annotation)
            args_list.append(f"{arg.arg}: {arg_type}")
        args_str = ", ".join(args_list)

        return_type = self._get_return_type(func.returns)

        if return_type:
            lines.append(f"fn {func.name}({args_str}) -> {return_type} {{")
        else:
            lines.append(f"fn {func.name}({args_str}) {{")

        lines.append("    // TODO: Implement function body")

        if return_type == "i32":
            lines.append("    0  // Placeholder return")
        elif return_type == "f64":
            lines.append("    0.0  // Placeholder return")
        elif return_type == "bool":
            lines.append("    false  // Placeholder return")
        elif return_type == "&str":
            lines.append('    ""  // Placeholder return')

        lines.append("}")
        lines.append("")

        return lines


class CompatibilityAnalyzer:
    """Analyzer for Python to Rust conversion compatibility."""

    def analyze(self, tree: ast.AST) -> list[str]:
        """Analyze Python AST for Rust compatibility issues."""
        issues = []

        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                issues.append(f"Import statement '{node.names[0].name}' needs manual conversion")
            elif isinstance(node, ast.ClassDef):
                issues.append(f"Class '{node.name}' needs manual conversion to Rust struct/impl")
            elif isinstance(node, ast.Try):
                issues.append("Try/except blocks need manual conversion to Rust error handling")
            elif isinstance(node, ast.Lambda):
                issues.append("Lambda functions need manual conversion")

        return issues


if __name__ == "__main__":
    main()
