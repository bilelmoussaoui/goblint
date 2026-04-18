#include <glib.h>

static void
test_multiple_same_line (const char *name)
{
  /* Multiple strcmp on same line */
  if (g_strcmp0 (name, ".") == 0 || g_strcmp0 (name, "..") == 0)
    g_print ("dot or dotdot\n");

  /* Three strcmp on same line */
  if (g_strcmp0 (name, "a") == 0 || g_strcmp0 (name, "b") == 0 || g_strcmp0 (name, "c") == 0)
    g_print ("a or b or c\n");
}
