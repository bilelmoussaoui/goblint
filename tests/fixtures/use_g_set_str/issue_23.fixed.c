#include <glib.h>

typedef struct {
  gchar *word;
} MyObj;

/* nested function calls as strdup arg — trim_end_matches eats extra parens */

static void
set_word (MyObj *self, const char *raw)
{
  g_set_str (&self->word, outer_func (inner_func (raw)));
}
