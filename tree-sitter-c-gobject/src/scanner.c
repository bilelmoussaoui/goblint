#include "tree_sitter/parser.h"
#include <string.h>

typedef enum {
    GOBJECT_MACRO_NAME,            /* G_DECLARE_* / G_DEFINE_* (not _WITH_CODE) */
    GOBJECT_MACRO_NAME_WITH_CODE,  /* G_DEFINE_*_WITH_CODE                      */
    GOBJECT_BEGIN_DECLS,           /* G_BEGIN_DECLS                              */
    GOBJECT_END_DECLS,             /* G_END_DECLS                                */
    MACRO_MODIFIER_NAME,           /* any other ALL_CAPS ident                  */
    GOBJECT_EXPORT_MACRO,          /* ALL_CAPS ident immediately before G_DECLARE_* or G_DEFINE_* */
} TokenType;

void *tree_sitter_c_gobject_external_scanner_create(void) { return NULL; }
void tree_sitter_c_gobject_external_scanner_destroy(void *payload) { (void)payload; }
void tree_sitter_c_gobject_external_scanner_reset(void *payload) { (void)payload; }
unsigned tree_sitter_c_gobject_external_scanner_serialize(void *payload, char *buffer) {
    (void)payload; (void)buffer;
    return 0;
}
void tree_sitter_c_gobject_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
    (void)payload; (void)buffer; (void)length;
}

static void skip_whitespace(TSLexer *lexer) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
           lexer->lookahead == '\n' || lexer->lookahead == '\r') {
        lexer->advance(lexer, true);
    }
}

/* Like skip_whitespace but uses advance(..., false) so it does NOT reset
 * token_start_position.  Must be used for lookahead that happens after
 * mark_end() has already pinned the token boundary. */
static void lookahead_skip_whitespace(TSLexer *lexer) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
           lexer->lookahead == '\n' || lexer->lookahead == '\r') {
        lexer->advance(lexer, false);
    }
}

/* Advance past a balanced argument list starting at '(' (already confirmed). */
static void skip_argument_list(TSLexer *lexer) {
    int depth = 0;
    while (lexer->lookahead) {
        char c = (char)lexer->lookahead;
        lexer->advance(lexer, false);
        if (c == '(') depth++;
        else if (c == ')') { if (--depth == 0) return; }
    }
}

/* Check whether what follows the already-read ALL_CAPS identifier is '->'
 * (expression context: type-cast macro like G_OBJECT_CLASS(x)->method).
 * The scanner resets its position on false return, so reads here are safe. */
static bool followed_by_arrow(TSLexer *lexer) {
    lookahead_skip_whitespace(lexer);
    if (lexer->lookahead == '(') {
        skip_argument_list(lexer);
        lookahead_skip_whitespace(lexer);
    }
    return lexer->lookahead == '-';
}

/* Peek ahead (after whitespace) to check whether the next ALL_CAPS identifier
 * is a GObject macro name (G_DECLARE_* or G_DEFINE_*).
 * Call mark_end() BEFORE calling this so the token boundary is already saved.
 * Returns true if followed by a GObject macro, false otherwise.
 * Advances the lexer (caller relies on mark_end for the correct token end). */
static bool followed_by_gobject_macro(TSLexer *lexer) {
    lookahead_skip_whitespace(lexer);

    char buf[256];
    int len = 0;
    while (len < 255 &&
           (lexer->lookahead == '_' ||
            (lexer->lookahead >= 'A' && lexer->lookahead <= 'Z') ||
            (lexer->lookahead >= '0' && lexer->lookahead <= '9'))) {
        buf[len++] = (char)lexer->lookahead;
        lexer->advance(lexer, false);
    }
    buf[len] = '\0';

    return (len >= 10 && strncmp(buf, "G_DECLARE_", 10) == 0) ||
           (len >= 9  && strncmp(buf, "G_DEFINE_",   9) == 0);
}

bool tree_sitter_c_gobject_external_scanner_scan(
    void *payload,
    TSLexer *lexer,
    const bool *valid_symbols
) {
    (void)payload;

    bool any_valid = valid_symbols[GOBJECT_MACRO_NAME]           ||
                     valid_symbols[GOBJECT_MACRO_NAME_WITH_CODE]  ||
                     valid_symbols[GOBJECT_BEGIN_DECLS]           ||
                     valid_symbols[GOBJECT_END_DECLS]             ||
                     valid_symbols[MACRO_MODIFIER_NAME]           ||
                     valid_symbols[GOBJECT_EXPORT_MACRO];
    if (!any_valid) return false;

    skip_whitespace(lexer);

    /* Must start with an uppercase letter or underscore */
    if (!((lexer->lookahead >= 'A' && lexer->lookahead <= 'Z') ||
          lexer->lookahead == '_')) {
        return false;
    }

    /* Consume only uppercase letters, digits, and underscores.
     * Stops at the first lowercase letter, which means CamelCase identifiers
     * like GObject only contribute their uppercase prefix. */
    char buf[256];
    int len = 0;
    while (len < 255 &&
           (lexer->lookahead == '_' ||
            (lexer->lookahead >= 'A' && lexer->lookahead <= 'Z') ||
            (lexer->lookahead >= '0' && lexer->lookahead <= '9'))) {
        buf[len++] = (char)lexer->lookahead;
        lexer->advance(lexer, false);
    }
    buf[len] = '\0';

    /* If the very next character is a lowercase letter the original identifier
     * is CamelCase (e.g. GObject, MyType) — not a macro, so bail out and let
     * the regular lexer produce an identifier token. */
    if (lexer->lookahead >= 'a' && lexer->lookahead <= 'z') {
        return false;
    }

    /* G_DEFINE_TYPE_EXTENDED — 5-arg variant with a code block as the last arg,
     * same structure as *_WITH_CODE macros. Must be checked before the general
     * G_DEFINE_* rule so the more-specific token wins. */
    if (valid_symbols[GOBJECT_MACRO_NAME_WITH_CODE] &&
        strcmp(buf, "G_DEFINE_TYPE_EXTENDED") == 0) {
        lexer->result_symbol = GOBJECT_MACRO_NAME_WITH_CODE;
        return true;
    }

    /* G_DEFINE_*_WITH_CODE — must be checked before the general G_DEFINE_* rule
     * so the more-specific token wins. */
    if (valid_symbols[GOBJECT_MACRO_NAME_WITH_CODE] &&
        len >= 9 && strncmp(buf, "G_DEFINE_", 9) == 0 &&
        len >= 10 && strncmp(buf + len - 10, "_WITH_CODE", 10) == 0) {
        lexer->result_symbol = GOBJECT_MACRO_NAME_WITH_CODE;
        return true;
    }

    /* G_DECLARE_* / G_DEFINE_* — GObject type-system macros */
    if (valid_symbols[GOBJECT_MACRO_NAME] &&
        ((len >= 10 && strncmp(buf, "G_DECLARE_", 10) == 0) ||
         (len >= 9  && strncmp(buf, "G_DEFINE_",   9) == 0))) {
        lexer->result_symbol = GOBJECT_MACRO_NAME;
        return true;
    }

    if (valid_symbols[GOBJECT_BEGIN_DECLS] && strcmp(buf, "G_BEGIN_DECLS") == 0) {
        lexer->result_symbol = GOBJECT_BEGIN_DECLS;
        return true;
    }

    if (valid_symbols[GOBJECT_END_DECLS] && strcmp(buf, "G_END_DECLS") == 0) {
        lexer->result_symbol = GOBJECT_END_DECLS;
        return true;
    }

    /* Pin the token boundary to the end of the ALL_CAPS identifier.  All
     * subsequent advances are look-ahead only. */
    lexer->mark_end(lexer);

    /* GOBJECT_EXPORT_MACRO: identifier immediately before G_DECLARE_* or G_DEFINE_* */
    if (valid_symbols[GOBJECT_EXPORT_MACRO] && len >= 1) {
        if (followed_by_gobject_macro(lexer)) {
            lexer->result_symbol = GOBJECT_EXPORT_MACRO;
            return true;
        }
        /* followed_by_gobject_macro() advanced into the next identifier.
         * After this point the lexer sits at whatever follows that identifier
         * (or at '(' if there was no next identifier). */
    }

    /* MACRO_MODIFIER_NAME: any remaining ALL_CAPS identifier that isn't
     * followed by '->' (which would indicate a type-cast expression). */
    if (valid_symbols[MACRO_MODIFIER_NAME] && len >= 1) {
        if (followed_by_arrow(lexer)) return false;
        lexer->result_symbol = MACRO_MODIFIER_NAME;
        return true;
    }

    return false;
}
