#include <glib.h>

typedef struct {
  char *description;
} Event;

void test_pointer_comparison (Event *event)
{
  char *escaped_description = g_markup_escape_text (event->description, -1);

  /* This is NOT a NULL check - it's comparing two pointers */
  /* Should NOT be flagged */
  if (escaped_description != event->description)
    g_free (escaped_description);

  /* This is also NOT a NULL check */
  char *str1 = "foo";
  char *str2 = "bar";
  if (str1 != str2)
    g_free (str1);

  /* This IS a NULL check and should be flagged */
  char *ptr = NULL;
  if (ptr != NULL)
    g_free (ptr);
}
