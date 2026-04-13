#include <glib.h>

static gboolean
my_func (const char *a,
         const char *b)
{
  /* Equality check — use_g_str_equal handles this, not us */
  if (strcmp (a, b) == 0)
    return TRUE;

  /* Ordering comparison — g_strcmp0 is the right fix */
  if (g_strcmp0 (a, b) < 0)
    return FALSE;

  /* strncmp — no NULL-safe drop-in replacement */
  if (strncmp (a, b, 3) == 0)
    return FALSE;

  return TRUE;
}
