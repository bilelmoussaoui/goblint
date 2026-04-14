#include <glib.h>

static void
test_misuse (const char *a, const char *b)
{
  /* Wrong: bare boolean check - strcmp returns 0 for equality! */
  if (strcmp (a, b))
    g_print ("different\n");

  /* Wrong: negated bare boolean check */
  if (!strcmp (a, b))
    g_print ("equal\n");

  /* Wrong: bare g_strcmp0 in condition */
  if (g_strcmp0 (a, b))
    g_print ("different\n");

  /* Wrong: negated g_strcmp0 */
  if (!g_strcmp0 (a, b))
    g_print ("equal\n");

  /* Correct: explicit comparison to 0 */
  if (strcmp (a, b) == 0)
    g_print ("equal\n");

  /* Correct: explicit comparison */
  if (strcmp (a, b) != 0)
    g_print ("different\n");
}
