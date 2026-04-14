#include <glib.h>

typedef struct {
  gchar *multichar_buffer_save;
  gchar *filter;
} MyObj;

static gboolean is_filter;

/* Comment between free and strdup — should flag and preserve the comment in the fix */

static void
update_with_comment (MyObj *self, const char *text)
{
  /* We have to keep an old copy of the text around in case the user cancels. */
  g_set_str (&self->multichar_buffer_save, text);
}

/* Non-expression statements between free and strdup — must NOT be flagged */

static void
update_with_intermediate (MyObj *self, const char *filter)
{
  g_clear_pointer (&self->filter, g_free);

  for (const gchar *p = filter; p[0] != '\0'; p = g_utf8_next_char (p))
    {
      if (p[0] == '?')
        {
          is_filter = TRUE;
          break;
        }
    }

  if (!is_filter)
    return;

  self->filter = g_strdup (filter);
}
