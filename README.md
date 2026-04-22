# goblint

A tree-sitter-based linter for GObject/C applications.

**goblint** = **G**Object **L**inter

## Usage

```bash
# Lint current directory with default config
goblint

# Lint specific directory
goblint /path/to/project

# Use custom config file
goblint --config my-lint.toml /path/to/project

# Verbose output
goblint -v

# List all available rules with their enabled/disabled status
goblint --list-rules

# Run only specific rules (overrides config)
goblint --only use_g_strcmp0 --only use_clear_functions

# Add custom ignore patterns
goblint --ignore "build/**" --ignore "tests/**"
```

## Available Rules

Browse all available rules at **https://bilelmoussaoui.github.io/goblint/** with descriptions, examples, and configuration options.

Run `goblint --list-rules` to see the current status of all rules in your terminal.

## Configuration

Create a `goblint.toml` file in your project root to configure rules, set minimum GLib version, and define per-rule ignore patterns.

You can also use inline comments to suppress specific violations:

```c
/* goblint-ignore-next-line: use_g_strlcpy */
strcpy(dst, src);
```

See [CONFIG.md](CONFIG.md) for complete configuration documentation.

## CI/CD Integration

### Container Image

goblint is available as a container image for easy CI/CD integration:

```bash
podman run --rm -v "$PWD:/workspace:Z" ghcr.io/bilelmoussaoui/goblint:latest
```

### GitHub Actions

Using the container image with GitHub Code Scanning:

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
    container:
      image: ghcr.io/bilelmoussaoui/goblint:latest
    permissions:
      security-events: write  # Required for uploading SARIF results

    steps:
      - uses: actions/checkout@v4

      - name: Run goblint
        run: goblint --format sarif > goblint.sarif

      - name: Upload SARIF results
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: goblint.sarif
          category: goblint
```

The results will appear in the "Security" tab under "Code scanning alerts" for your repository, and as inline comments on pull requests.

### GitLab CI

Using the container image with GitLab's SARIF ingestion:

```yaml
goblint:
  stage: lint
  image: ghcr.io/bilelmoussaoui/goblint:latest
  script:
    - goblint --format sarif > goblint.sarif
  artifacts:
    reports:
      sarif: goblint.sarif
```

The results will appear in the merge request's security report and as inline comments.

### Installation Alternative

If you prefer installing locally instead of using containers:

```bash
cargo install --git https://github.com/bilelmoussaoui/goblint goblint
```

## LSP Server

For real-time linting in your editor:

```bash
cargo build --release --bin goblint-lsp
```

**Neovim** (nvim-lspconfig):
```lua
require('lspconfig.configs').gobject_lsp = {
  default_config = {
    cmd = {'goblint-lsp'},
    filetypes = {'c', 'h'},
    root_dir = require('lspconfig.util').root_pattern('goblint.toml', '.git'),
  },
}
require('lspconfig').gobject_lsp.setup{}
```

**VS Code**: Use a generic LSP client extension pointing to `goblint-lsp`

**Helix** (`~/.config/helix/languages.toml`):
```toml
[[language]]
name = "c"
language-servers = ["clangd", "goblint-lsp"]

[language-server.goblint-lsp]
command = "goblint-lsp"
```

Co-Authored by Claude Code.
