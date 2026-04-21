#include <glib-object.h>

enum {
  PROP_0,
  PROP_NAME,
  PROP_DISPLAY_NAME,
  N_PROPS
};

static GParamSpec *props[N_PROPS] = { NULL, };

static void
foo_class_init (FooClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  props[PROP_NAME] = g_param_spec_string ("name", NULL, NULL,
                                          NULL, G_PARAM_READWRITE);

  props[PROP_DISPLAY_NAME] = g_param_spec_string ("display-name", NULL, NULL,
                                                   NULL, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, N_PROPS, props);
}

static void
foo_set_name (FooObject *self, const char *name)
{
  g_object_notify (G_OBJECT (self), "name");
  g_object_notify (G_OBJECT (self), "display-name");
}
