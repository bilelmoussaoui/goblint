#include "tree_sitter/parser.h"
#include <string.h>

typedef enum {
    GOBJECT_MACRO_NAME,            /* G_DECLARE_* / G_DEFINE_* (not _WITH_CODE) */
    GOBJECT_MACRO_NAME_WITH_CODE,  /* G_DEFINE_*_WITH_CODE                      */
    GOBJECT_BEGIN_DECLS,           /* G_BEGIN_DECLS                              */
    GOBJECT_END_DECLS,             /* G_END_DECLS                                */
    MACRO_MODIFIER_NAME,           /* any other ALL_CAPS ident                  */
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
    skip_whitespace(lexer);
    if (lexer->lookahead == '(') {
        skip_argument_list(lexer);
        skip_whitespace(lexer);
    }
    return lexer->lookahead == '-';
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
                     valid_symbols[MACRO_MODIFIER_NAME];
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

    /* Any remaining all-caps identifier is a potential macro modifier
     * (CLUTTER_EXPORT, G_MODULE_EXPORT, G_DEPRECATED, …).
     * But if what follows (after any argument list) is '->' the identifier is
     * a type-cast macro used in an expression (e.g. G_OBJECT_CLASS(x)->method),
     * not a declaration modifier.  Return false so the regular lexer handles it
     * as a plain identifier; tree-sitter resets the lexer position on false. */
    if (valid_symbols[MACRO_MODIFIER_NAME] && len >= 1) {
        if (followed_by_arrow(lexer)) return false;
        lexer->result_symbol = MACRO_MODIFIER_NAME;
        return true;
    }

    return false;
}
