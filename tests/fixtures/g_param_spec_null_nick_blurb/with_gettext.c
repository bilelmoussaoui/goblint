#include <glib-object.h>

static void
foo_class_init (FooClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  /* Translation macros should be detected as violations */
  g_object_class_install_property (object_class, PROP_USERNAME,
    g_param_spec_string ("username", _("Username"), _("The username"),
                         NULL, G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS));

  g_object_class_install_property (object_class, PROP_HOSTNAME,
    g_param_spec_string ("hostname", _("Hostname"), _("The hostname"),
                         NULL, G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS));
}
