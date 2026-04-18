#include <glib.h>

static void
test_multiple_same_line (const char *a, const char *b, const char *c)
{
  /* Multiple bare strcmp on same line */
  if (strcmp (a, b) || strcmp (b, c))
    g_print ("different\n");

  /* Multiple negated strcmp on same line */
  if (!strcmp (a, b) && !strcmp (b, c))
    g_print ("all equal\n");

  /* Mixed: bare and negated on same line */
  if (strcmp (a, b) && !strcmp (b, c))
    g_print ("mixed\n");
}
