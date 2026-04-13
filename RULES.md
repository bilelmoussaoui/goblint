# Available Rules

All rules are enabled by default. Run `goblint --list-rules` to see their current status.

## Correctness

Rules that detect code that is outright wrong or very useless.

- **g_error_init** - Ensure GError* variables are initialized to NULL
- **property_enum_zero** - Ensure property enums start with PROP_0, not PROP_NAME = 0
- **g_param_spec_static_name_canonical** - Ensure property names are canonical (use dashes, not underscores). Critical with G_PARAM_STATIC_NAME
- **g_object_virtual_methods_chain_up** - Ensure dispose/finalize/constructed methods chain up to parent class
- **use_g_ascii_functions** - Use g_ascii_* functions instead of locale-dependent C ctype functions (tolower, toupper, isdigit, etc.)
- **use_g_strlcpy** - Avoid unsafe string functions (strcpy, strcat, strncat); use g_strlcpy/g_strlcat instead

## Suspicious

Rules that detect code that is most likely wrong or useless.

- **missing_implementation** - Report functions declared in headers but not implemented
- **g_task_source_tag** - Ensure g_task_set_source_tag is called after g_task_new
- **unnecessary_null_check** - Detect unnecessary NULL checks before g_free/g_clear_* functions

## Style

Rules that suggest more idiomatic ways to write code.

- **use_g_settings_typed** - Prefer g_settings_get/set_string/boolean/etc over g_settings_get/set_value with g_variant
- **use_g_variant_new_typed** - Prefer g_variant_new_string/boolean/etc over g_variant_new with format strings
- **use_g_strcmp0** - Use g_strcmp0 instead of strcmp (NULL-safe)
- **use_explicit_default_flags** - Use explicit default flag constants (e.g., G_APPLICATION_DEFAULT_FLAGS) instead of 0
- **use_g_str_equal** - Suggest g_str_equal() instead of strcmp() == 0 for better readability
- **use_g_string_free_and_steal** - Suggests g_string_free_and_steal instead of g_string_free (..., FALSE) for better readability
- **use_g_source_once** - Suggest using g_idle_add_once/g_timeout_add_once when callback always returns G_SOURCE_REMOVE
- **use_g_source_constants** - Use G_SOURCE_CONTINUE/G_SOURCE_REMOVE instead of TRUE/FALSE in GSourceFunc callbacks
- **use_g_steal_pointer** - Use g_steal_pointer() instead of manually copying a pointer and setting it to NULL
- **use_g_str_has_prefix_suffix** - Use g_str_has_prefix/g_str_has_suffix() instead of manual strncmp/strcmp comparisons

## Complexity

Rules that suggest simpler alternatives to complex patterns.

- **use_clear_functions** - Suggest g_clear_object/g_clear_pointer instead of manual unref and NULL assignment
- **use_g_new** - Suggest g_new/g_new0 instead of g_malloc/g_malloc0 with sizeof for type safety
- **use_g_set_str** - Suggest g_set_str() instead of manual g_free and g_strdup
- **use_g_autoptr_error** - Suggest g_autoptr(GError) instead of manual g_error_free
- **use_g_autoptr_goto_cleanup** - Suggest g_autoptr instead of goto error cleanup pattern
- **use_g_autoptr_inline_cleanup** - Suggest g_autoptr instead of inline manual cleanup (g_object_unref/g_free)
- **use_g_autofree** - Suggest g_autofree for string/buffer types instead of manual g_free
- **use_g_clear_handle_id** - Suggest g_clear_handle_id instead of manual cleanup and zero assignment
- **use_g_clear_list** - Suggest g_clear_list/g_clear_slist instead of manual g_list_free/g_slist_free and NULL assignment
- **use_g_clear_weak_pointer** - Suggest g_clear_weak_pointer instead of manual g_object_remove_weak_pointer and NULL assignment
- **use_g_file_load_bytes** - Suggest g_file_load_bytes instead of g_file_load_contents + g_bytes_new_take
- **use_g_object_new_with_properties** - Suggest setting properties in g_object_new instead of separate g_object_set calls

## Perf

Rules that suggest changes for better performance.

- **g_param_spec_static_strings** - Ensure g_param_spec_* calls use G_PARAM_STATIC_STRINGS flag for string literals
- **use_g_value_set_static_string** - Use g_value_set_static_string for string literals instead of g_value_set_string
- **use_g_object_notify_by_pspec** - Suggest g_object_notify_by_pspec instead of g_object_notify for better performance

## Pedantic

Rules that are rather strict or have occasional false positives.

- **g_declare_semicolon** - Enforce semicolons after G_DECLARE_* macros
- **matching_declare_define** - Ensure G_DECLARE_* and G_DEFINE_* macros are used consistently
- **g_param_spec_null_nick_blurb** - Ensure g_param_spec_* functions have NULL for nick and blurb parameters
- **use_g_object_class_install_properties** - Suggest g_object_class_install_properties for multiple g_object_class_install_property calls

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
