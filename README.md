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

# List all available rules with their enabled/disabled status
gobject-lint --list-rules

# Run only specific rules (overrides config)
gobject-lint --only use_g_strcmp0 --only use_clear_functions

# Add custom ignore patterns
gobject-lint --ignore "build/**" --ignore "tests/**"
```

## Available Rules

See [RULES.md](RULES.md) for a complete list of all available rules organized by category.

Run `gobject-lint --list-rules` to see the current status of all rules.

## Configuration

Create a `gobject-lint.toml` file in your project root:

```toml
# Minimum supported GLib version (optional)
# Rules requiring newer GLib versions will be automatically disabled
min_glib_version = "2.40"

[rules]
# Ensure g_param_spec_* functions have NULL for nick and blurb parameters
g_param_spec_null_nick_blurb = true
```

See gobject-lint.toml for all the supported rules/configurations.

## CI/CD Integration

### GitHub Actions

Integrate gobject-lint with GitHub Code Scanning using SARIF output:

```yaml
name: GObject Lint

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  lint:
    runs-on: ubuntu-latest
    permissions:
      security-events: write  # Required for uploading SARIF results

    steps:
      - uses: actions/checkout@v6

      - name: Install gobject-lint
        run: |
          cargo install --git https://github.com/bilelmoussaoui/gobject-lint gobject-lint

      - name: Run gobject-lint
        run: |
          gobject-lint --format sarif > gobject-lint.sarif
        continue-on-error: true  # Don't fail the workflow on lint errors

      - name: Upload SARIF results
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: gobject-lint.sarif
          category: gobject-lint
```

The results will appear in the "Security" tab under "Code scanning alerts" for your repository, and as inline comments on pull requests.

You can also filter by category:

```bash
# Run only correctness rules for critical checks
gobject-lint --category correctness --format sarif > results.sarif

# Run only performance rules
gobject-lint --category perf --format sarif > results.sarif
```

## LSP Server

For real-time linting in your editor:

```bash
cargo build --release --bin gobject-lsp
```

**Neovim** (nvim-lspconfig):
```lua
require('lspconfig.configs').gobject_lsp = {
  default_config = {
    cmd = {'gobject-lsp'},
    filetypes = {'c', 'h'},
    root_dir = require('lspconfig.util').root_pattern('gobject-lint.toml', '.git'),
  },
}
require('lspconfig').gobject_lsp.setup{}
```

**VS Code**: Use a generic LSP client extension pointing to `gobject-lsp`

**Helix** (`~/.config/helix/languages.toml`):
```toml
[[language]]
name = "c"
language-servers = ["clangd", "gobject-lsp"]

[language-server.gobject-lsp]
command = "gobject-lsp"
```

Co-Authored by Claude Code.
