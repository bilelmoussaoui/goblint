#include <glib.h>

static void
test_variable_assignment (const char *a, const char *b)
{
  /* Correct: strcmp assigned to variable, then used in comparison */
  int cmp = strcmp (a, b);

  if (cmp == 0)
    g_print ("equal\n");

  if (cmp != 0)
    g_print ("different\n");

  /* Correct: multiple strcmp assigned to variables */
  int result1 = strcmp (a, "foo");
  int result2 = g_strcmp0 (b, "bar");

  if (result1 == 0 && result2 != 0)
    g_print ("match\n");
}
