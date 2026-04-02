# gobject-lint

A tree-sitter-based linter for GObject/C applications.

## Usage

```bash
# Lint current directory with default config
gobject-lint

# Lint specific directory
gobject-lint /path/to/project

# Use custom config file
gobject-lint --config my-lint.toml /path/to/project

# Verbose output
gobject-lint -v
```

## Configuration

Create a `gobject-lint.toml` file in your project root:

```toml
[rules]
# Ensure g_param_spec_* functions have NULL for nick and blurb parameters
g_param_spec_null_nick_blurb = true
```

See gobject-lint.toml for all the supported rules/configurations.

Co-Authored by Claude Code.
