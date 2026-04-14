#include <glib.h>

static gboolean
my_func (const char *a,
         const char *b)
{
  /* Equality check with proper comparison */
  if (strcmp (a, b) == 0)
    return TRUE;

  /* Ordering comparison — g_strcmp0 is the right fix */
  if (strcmp (a, b) < 0)
    return FALSE;

  /* strncmp — no NULL-safe drop-in replacement */
  if (strncmp (a, b, 3) == 0)
    return FALSE;

  return TRUE;
}
