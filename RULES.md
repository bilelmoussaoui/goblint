# Available Rules

All rules are enabled by default. Run `goblint --list-rules` to see their current status.

## Per-Rule Configuration

Some rules support additional configuration options beyond `level` and `ignore`. These are documented in the rule descriptions below.

## Correctness

Rules that detect code that is outright wrong or very useless.

- **g_error_init** - Ensure GError* variables are initialized to NULL
- **g_error_leak** - Check for GError variables that are neither freed nor propagated
- **g_param_spec_static_name_canonical** - Ensure property names are canonical (use dashes, not underscores). Critical with G_PARAM_STATIC_NAME
- **g_object_virtual_methods_chain_up** - Ensure dispose/finalize/constructed methods chain up to parent class
- **property_enum_coverage** - Ensure all property enum values have corresponding g_param_spec or g_object_class_override_property
- **strcmp_explicit_comparison** - Require explicit comparison with 0 for strcmp/g_strcmp0 (returns 0 for equality, not TRUE)
- **use_g_ascii_functions** - Use g_ascii_* functions instead of locale-dependent C ctype functions (tolower, toupper, isdigit, etc.)
- **use_g_strlcpy** - Avoid unsafe string functions (strcpy, strcat, strncat); use g_strlcpy/g_strlcat instead

## Suspicious

Rules that detect code that is most likely wrong or useless.

- **missing_implementation** - Report functions declared in headers but not implemented
- **unnecessary_null_check** - Detect unnecessary NULL checks before g_free/g_clear_* functions
- **g_source_id_not_stored** - Warn when g_timeout_add/g_idle_add are called without storing the returned source ID (prevents use-after-free)

## Style

Rules that suggest more idiomatic ways to write code.

- **property_enum_convention** - Modernize property enum pattern: remove PROP_0/N_PROPS sentinels, start from = 1, use LAST_PROP + 1 for array size, and G_N_ELEMENTS for install_properties.
- **include_order** - Enforce consistent include ordering: config.h, associated header, standard C headers, system headers (<>), project headers ("") (all alphabetically sorted within each group, blank line between groups)
  - **Per-rule config option `config_header`**: Customize the config header filename (default: `"config.h"`). Example:
    ```toml
    [rules.include_order]
    level = "error"
    config_header = "myproject-config.h"
    ```
- **use_pragma_once** - Suggest #pragma once instead of traditional include guards (#ifndef/#define/#endif)
- **use_g_settings_typed** - Prefer g_settings_get/set_string/boolean/etc over g_settings_get/set_value with g_variant
- **use_g_variant_new_typed** - Prefer g_variant_new_string/boolean/etc over g_variant_new with format strings
- **use_g_strcmp0** - Suggest g_strcmp0 instead of strcmp if arguments can be NULL (g_strcmp0 is NULL-safe)
- **use_explicit_default_flags** - Use explicit default flag constants (e.g., G_APPLICATION_DEFAULT_FLAGS) instead of 0
- **use_g_string_free_and_steal** - Suggests g_string_free_and_steal instead of g_string_free (..., FALSE) for better readability
- **use_g_source_constants** - Use G_SOURCE_CONTINUE/G_SOURCE_REMOVE instead of TRUE/FALSE in GSourceFunc callbacks
- **use_g_steal_pointer** - Use g_steal_pointer() instead of manually copying a pointer and setting it to NULL
- **use_g_str_has_prefix_suffix** - Use g_str_has_prefix/g_str_has_suffix() instead of manual strncmp/strcmp comparisons

## Complexity

Rules that suggest simpler alternatives to complex patterns.

- **use_clear_functions** - Suggest g_clear_object/g_clear_pointer instead of manual unref and NULL assignment
- **use_g_bytes_unref_to_data** - Suggest g_bytes_unref_to_data() instead of g_bytes_get_data() followed by g_bytes_unref()
- **use_g_new** - Suggest g_new/g_new0 instead of g_malloc/g_malloc0 with sizeof for type safety
- **use_g_set_str** - Suggest g_set_str() instead of manual g_free and g_strdup
- **use_g_autoptr_error** - Suggest g_autoptr(GError) instead of manual g_error_free
- **use_g_autoptr_goto_cleanup** - Suggest g_autoptr instead of goto error cleanup pattern
- **use_g_autoptr_inline_cleanup** - Suggest g_autoptr instead of inline manual cleanup (g_object_unref/g_free)
  - **Per-rule config option `ignore_types`**: List of glob patterns for types to ignore (default: `[]`). Use this to skip types that don't work well with g_autoptr (e.g., Cairo, Pango types). Example:
    ```toml
    [rules.use_g_autoptr_inline_cleanup]
    level = "error"
    ignore_types = ["cairo_*", "Pango*", "RsvgHandle"]
    ```
- **use_g_autofree** - Suggest g_autofree for string/buffer types instead of manual g_free
- **use_g_clear_handle_id** - Suggest g_clear_handle_id instead of manual cleanup and zero assignment
- **use_g_clear_signal_handler** - Use g_clear_signal_handler() instead of g_signal_handler_disconnect() and zeroing the ID
- **use_g_clear_list** - Suggest g_clear_list/g_clear_slist instead of manual g_list_free/g_slist_free and NULL assignment
- **use_g_clear_weak_pointer** - Suggest g_clear_weak_pointer instead of manual g_object_remove_weak_pointer and NULL assignment
- **use_g_file_load_bytes** - Suggest g_file_load_bytes instead of g_file_load_contents + g_bytes_new_take
- **use_g_source_once** - Suggest using g_idle_add_once/g_timeout_add_once/g_timeout_add_seconds_once when callback always returns G_SOURCE_REMOVE
- **use_g_object_new_with_properties** - Suggest setting properties in g_object_new instead of separate g_object_set calls
- **use_g_object_class_install_properties** - Suggest g_object_class_install_properties for multiple g_object_class_install_property calls

## Perf

Rules that suggest changes for better performance.

- **g_param_spec_static_strings** - Ensure *_param_spec_* calls use G_PARAM_STATIC_STRINGS flag for string literals
  - **Per-rule config option `static_flags`**: List of custom flag constants that already include `G_PARAM_STATIC_STRINGS` (default: `[]`). Use this if your project has custom macros like `ST_PARAM_READWRITE` that already include the static strings flag. Example:

    ```toml
    [rules.g_param_spec_static_strings]
    level = "error"
    static_flags = ["ST_PARAM_READWRITE", "ST_PARAM_READABLE"]
    ```

- **use_g_value_set_static_string** - Use g_value_set_static_string for string literals instead of g_value_set_string
- **use_g_object_notify_by_pspec** - Suggest g_object_notify_by_pspec instead of g_object_notify for better performance

## Pedantic

Rules that are rather strict or have occasional false positives.

- **g_declare_semicolon** - Enforce semicolons after G_DECLARE_* and G_DEFINE_* macros (including multi-line variants)
- **g_param_spec_null_nick_blurb** - Ensure g_param_spec_* functions have NULL for nick and blurb parameters
  - **Per-rule config option `static_flags`**: List of custom flag constants that include `G_PARAM_STATIC_STRINGS`. If your project uses custom flags like `ST_PARAM_READWRITE` with non-NULL nick/blurb, this rule will skip those calls. Example:

    ```toml
    [rules.g_param_spec_null_nick_blurb]
    level = "warn"
    static_flags = ["ST_PARAM_READWRITE"]
    ```

- **g_task_source_tag** - Ensure g_task_set_source_tag is called after g_task_new (useful for debugging, but not required)
- **matching_declare_define** - Ensure G_DECLARE_* and G_DEFINE_* macros are used consistently
- **untranslated_string** - Detect user-visible strings in GTK/Adwaita functions that should be wrapped with gettext (use inline ignore for strings that don't need translation)

## Restriction

Rules that prevent the use of deprecated language/library features.

- **deprecated_add_private** - Detect deprecated g_type_class_add_private (use G_DEFINE_TYPE_WITH_PRIVATE instead)

## Filtering by Category

You can filter rules by category using the `--category` flag:

```bash
# Run only correctness rules
goblint --category correctness

# Run only performance rules
goblint --category perf

# List only style rules
goblint --list-rules --category style
```

Available categories: `correctness`, `suspicious`, `style`, `complexity`, `perf`, `pedantic`, `restriction`
