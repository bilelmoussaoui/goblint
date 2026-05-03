#include <glib-object.h>

/* Signals enum appears first - must not be confused with the property enum */
enum {
  SYNC_MESSAGE,
  ASYNC_MESSAGE,
  LAST_SIGNAL
};

enum {
  PROP_0,
  PROP_ENABLE_ASYNC
};

static void
foo_class_init (FooClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  g_object_class_install_property (object_class, PROP_ENABLE_ASYNC,
      g_param_spec_boolean ("enable-async", NULL, NULL, TRUE,
          G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS));
}
