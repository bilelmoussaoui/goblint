#include <glib-object.h>

/* N_PROPS in middle (override properties) - should NOT be transformed */
typedef enum {
  PROP_0,
  PROP_PASSWORD_VISIBLE,
  PROP_CONFIRM_VISIBLE,
  N_PROPS,
  /* GcrPrompt - override properties from interface */
  PROP_TITLE,
  PROP_MESSAGE,
  PROP_DESCRIPTION
} OverrideProperty;

static GParamSpec *override_props[N_PROPS] = { NULL, };

static void
override_class_init (OverrideClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  override_props[PROP_PASSWORD_VISIBLE] = g_param_spec_boolean ("password-visible", NULL, NULL, FALSE, G_PARAM_READWRITE);
  override_props[PROP_CONFIRM_VISIBLE] = g_param_spec_boolean ("confirm-visible", NULL, NULL, FALSE, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, N_PROPS, override_props);

  /* Override properties from GcrPrompt interface */
  g_object_class_override_property (object_class, PROP_TITLE, "title");
  g_object_class_override_property (object_class, PROP_MESSAGE, "message");
  g_object_class_override_property (object_class, PROP_DESCRIPTION, "description");
}
