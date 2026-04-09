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

All rules are enabled by default. Run `gobject-lint --list-rules` to see their current status.

- **gdeclare_semicolon** - Enforce semicolons after G_DECLARE_* macros
- **missing_implementation** - Report functions declared in headers but not implemented
- **deprecated_add_private** - Detect deprecated g_type_class_add_private (use G_DEFINE_TYPE_WITH_PRIVATE instead)
- **prefer_g_new** - Suggest g_new/g_new0 instead of g_malloc/g_malloc0 with sizeof for type safety
- **use_g_strcmp0** - Use g_strcmp0 instead of strcmp (NULL-safe)
- **use_clear_functions** - Suggest g_clear_object/g_clear_pointer instead of manual unref and NULL assignment
- **g_param_spec_null_nick_blurb** - Ensure g_param_spec_* functions have NULL for nick and blurb parameters
- **gerror_init** - Ensure GError* variables are initialized to NULL
- **property_enum_zero** - Ensure property enums start with PROP_0, not PROP_NAME = 0
- **dispose_finalize_chains_up** - Ensure dispose/finalize methods chain up to parent class
- **gtask_source_tag** - Ensure g_task_set_source_tag is called after g_task_new
- **unnecessary_null_check** - Detect unnecessary NULL checks before g_free/g_clear_pointer
- **strcmp_for_string_equal** - Suggest g_str_equal() instead of strcmp() == 0 for better readability
- **use_g_set_str** - Suggest g_set_str() instead of manual g_free and g_strdup
- **suggest_g_autoptr_error** - Suggest g_autoptr(GError) instead of manual g_error_free
- **suggest_g_autoptr_goto_cleanup** - Suggest g_autoptr instead of goto error cleanup pattern
- **suggest_g_autoptr_inline_cleanup** - Suggest g_autoptr instead of inline manual cleanup (g_object_unref/g_free)
- **suggest_g_autofree** - Suggest g_autofree for string/buffer types instead of manual g_free
- **use_g_clear_handle_id** - Suggest g_clear_handle_id instead of manual cleanup and zero assignment
- **use_g_clear_list** - Suggest g_clear_list/g_clear_slist instead of manual g_list_free/g_slist_free and NULL assignment
- **use_g_object_notify_by_pspec** - Suggest g_object_notify_by_pspec instead of g_object_notify for better performance
- **use_g_string_free_and_steal** - Suggests g_string_free_and_steal instead of g_string_free (..., FALSE) for better readability

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
