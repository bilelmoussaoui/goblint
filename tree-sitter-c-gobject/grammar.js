/**
 * @file GObject-aware C grammar for tree-sitter, extending tree-sitter-c
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const C = require('./tree-sitter-c/grammar');

module.exports = grammar(C, {
  name: 'c_gobject',

  externals: $ => [
    $._gobject_macro_name,            // G_DECLARE_* / G_DEFINE_* (not _WITH_CODE)
    $._gobject_macro_name_with_code,  // G_DEFINE_*_WITH_CODE
    $._gobject_begin_decls,           // G_BEGIN_DECLS
    $._gobject_end_decls,             // G_END_DECLS
    $._macro_modifier_name,           // any other ALL_CAPS identifier (CLUTTER_EXPORT etc.)
    $._gobject_export_macro,          // ALL_CAPS macro immediately preceding G_DECLARE_*/G_DEFINE_*
  ],

  conflicts: ($, original) => [
    ...original,
    [$.macro_modifier, $.type_specifier],
    [$.macro_modifier, $._declarator],
    [$.macro_modifier, $.macro_type_specifier],
    // Inside *_WITH_CODE macros: an identifier followed by '(' could be the last
    // regular arg (expression) or the first code-block item (gobject_code_block_item).
    // GLR resolves by looking at what follows the argument list — ',' means
    // regular arg, anything else means code block.
    [$.expression, $.gobject_code_block_item],
  ],

  rules: {
    // Allow macro modifiers (G_GNUC_CONST, G_GNUC_DEPRECATED, G_GNUC_DEPRECATED_FOR(...), etc.)
    // after the parameter list in function declarations.
    // The upstream rule only allows attribute_specifier (__attribute__((...))), but GLib/GObject
    // code uses ALL_CAPS macro shorthands that our macro_modifier rule already handles.
    _function_declaration_declarator: ($, _original) => prec.right(1,
      seq(
        field('declarator', $._declarator),
        field('parameters', $.parameter_list),
        optional($.gnu_asm_expression),
        repeat(choice(
          $.attribute_specifier,
          $.macro_modifier,
        )),
      )
    ),

    _top_level_item: ($, original) => choice(
      $.gobject_type_macro,
      $.gobject_decls_block,
      $.gobject_macro_statement,
      original,
    ),

    _block_item: ($, original) => choice(
      $.gobject_type_macro,
      $.gobject_decls_block,
      $.gobject_macro_statement,
      original,
    ),

    _declaration_modifiers: ($, original) => choice(
      original,
      $.macro_modifier,
    ),

    gobject_decls_block: $ => seq(
      $._gobject_begin_decls,
      repeat($._top_level_item),
      $._gobject_end_decls,
    ),

    gobject_type_macro: $ => choice(
      // Standard macros (G_DECLARE_*, G_DEFINE_TYPE, etc.): all args comma-separated.
      // Optional leading export/deprecation modifiers (META_EXPORT, CLUTTER_EXPORT, …).
      // Uses _gobject_export_macro (not macro_modifier) so the scanner only produces
      // this token when peek-ahead confirms a G_DECLARE_*/G_DEFINE_* follows — no
      // risk of the token leaking into G_DEFINE_*_WITH_CODE code-block bodies.
      seq(repeat($._gobject_export_macro), $._gobject_macro_name, $.argument_list),

      // *_WITH_CODE macros: N comma-terminated regular args followed by a
      // whitespace-separated code block (G_ADD_PRIVATE, G_IMPLEMENT_INTERFACE, …).
      // Using a dedicated external token lets the scanner disambiguate at lex time
      // so no GLR conflicts arise.
      seq(
        repeat($._gobject_export_macro),
        $._gobject_macro_name_with_code,
        '(',
        repeat1(seq($.expression, ',')),
        optional($.gobject_code_block),
        ')',
      ),
    ),

    // Whitespace-separated sequence of macro calls inside a *_WITH_CODE code
    // block.  No commas between items — that's the GLib convention.
    // Each item is always an ALL-CAPS identifier followed by an argument list
    // (G_ADD_PRIVATE, G_IMPLEMENT_INTERFACE, …).  Using identifier+argument_list
    // directly (rather than the full call_expression rule) avoids GLR conflicts
    // with chained-call expressions.
    gobject_code_block: $ => repeat1($.gobject_code_block_item),

    gobject_code_block_item: $ => seq(
      $.identifier,
      $.argument_list,
    ),

    // Standalone macro call followed by semicolon: G_STATIC_ASSERT(...), etc.
    // Handles cases where an ALL_CAPS macro (parsed as macro_modifier) appears
    // as a standalone statement rather than a declaration modifier.
    gobject_macro_statement: $ => seq(
      $.macro_modifier,
      ';',
    ),

    // Export / deprecation / availability macros used as declaration modifiers.
    // Simple: CLUTTER_EXPORT, G_DEPRECATED, G_UNAVAILABLE
    // Function-like: G_DEPRECATED_FOR(...), GLIB_AVAILABLE_IN_2_80(...)
    // Uses an external token so only ALL_CAPS identifiers match, not CamelCase type names.
    macro_modifier: $ => prec.left(2, seq(
      $._macro_modifier_name,
      optional($.argument_list),
    )),
  },
});
