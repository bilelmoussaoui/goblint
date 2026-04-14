#include <glib-object.h>

static void
foo_class_init (FooClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  /* Both nick and blurb are string literals, no static flags */
  g_object_class_install_property (object_class, PROP_A,
    g_param_spec_string ("prop-a", "Prop A", "The prop-a", NULL, G_PARAM_READWRITE));

  /* Both nick and blurb are string literals, has G_PARAM_STATIC_STRINGS */
  g_object_class_install_property (object_class, PROP_B,
    g_param_spec_string ("prop-b", "Prop B", "The prop-b", NULL,
                         G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS));

  /* Both nick and blurb are string literals, has individual static flags */
  g_object_class_install_property (object_class, PROP_C,
    g_param_spec_string ("prop-c", "Prop C", "The prop-c", NULL,
                         G_PARAM_READWRITE | G_PARAM_STATIC_NAME | G_PARAM_STATIC_NICK | G_PARAM_STATIC_BLURB));

  /* Only nick is a string literal, blurb is NULL, no static flags */
  g_object_class_install_property (object_class, PROP_D,
    g_param_spec_string ("prop-d", "Prop D", NULL, NULL, G_PARAM_READWRITE));

  /* Only blurb is a string literal, nick is NULL, no static flags */
  g_object_class_install_property (object_class, PROP_E,
    g_param_spec_string ("prop-e", NULL, "The prop-e", NULL, G_PARAM_READWRITE));

  /* Only nick is a string literal, has G_PARAM_STATIC_NAME: flags already correct after fix */
  g_object_class_install_property (object_class, PROP_F,
    g_param_spec_string ("prop-f", "Prop F", NULL, NULL,
                         G_PARAM_READWRITE | G_PARAM_STATIC_NAME));

  /* Both are already NULL: no violation */
  g_object_class_install_property (object_class, PROP_G,
    g_param_spec_string ("prop-g", NULL, NULL, NULL,
                         G_PARAM_READWRITE | G_PARAM_STATIC_NAME));
}
