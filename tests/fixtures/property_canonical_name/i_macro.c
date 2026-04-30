#include <glib-object.h>

/* Property name via I_() with no underscores — should not be flagged. */
static void
foo_class_init (FooClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  obj_properties[PROP_STACK] =
    g_param_spec_object (I_("stack"), NULL, NULL,
                         GTK_TYPE_STACK,
                         G_PARAM_READWRITE|G_PARAM_STATIC_STRINGS|G_PARAM_EXPLICIT_NOTIFY);
}
