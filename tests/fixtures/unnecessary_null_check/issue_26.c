#include <glib.h>

typedef struct {
  guint   autohide_timeout_id;
  gchar  *name;
} MyObj;

static void update_autohide (MyObj *self, gboolean val) {}

/* if/else — the else branch has real logic, must NOT be flagged */

static void
cancel_autohide (MyObj *self)
{
  if (self->autohide_timeout_id != 0)
    g_clear_handle_id (&self->autohide_timeout_id, g_source_remove);
  else
    update_autohide (self, TRUE);
}

/* braced body with else — also must NOT be flagged */

static void
cancel_autohide_braced (MyObj *self)
{
  if (self->name != NULL)
    {
      g_free (self->name);
    }
  else
    {
      self->name = g_strdup ("default");
    }
}
