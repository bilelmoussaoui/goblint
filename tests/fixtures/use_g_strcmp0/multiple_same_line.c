#include <glib.h>

static void
test_multiple_same_line (const char *name)
{
  /* Multiple strcmp on same line */
  if (strcmp (name, ".") == 0 || strcmp (name, "..") == 0)
    g_print ("dot or dotdot\n");

  /* Three strcmp on same line */
  if (strcmp (name, "a") == 0 || strcmp (name, "b") == 0 || strcmp (name, "c") == 0)
    g_print ("a or b or c\n");
}
