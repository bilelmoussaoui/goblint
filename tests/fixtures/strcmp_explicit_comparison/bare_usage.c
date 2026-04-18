#include <glib.h>

static void
test_bare_usage (const char *a, const char *b)
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

/* Correct: return value for comparison function */
static gint
compare_func (gconstpointer a, gconstpointer b)
{
  const char *str_a = (const char *) a;
  const char *str_b = (const char *) b;

  return g_strcmp0 (str_a, str_b);
}
