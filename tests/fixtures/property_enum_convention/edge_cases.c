#include <glib-object.h>

/* Case 1: First property already has = 0 with bad spacing */
typedef enum {
  PROP_0,
  PROP_BOUNDING_BOX= 0,
  PROP_CHILD,
  N_PROPS
} BadSpacingProperty;

static GParamSpec *bad_spacing_props[N_PROPS] = { NULL, };

static void
bad_spacing_class_init (BadSpacingClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  bad_spacing_props[PROP_BOUNDING_BOX] = g_param_spec_int ("bounding-box", NULL, NULL, 0, 100, 0, G_PARAM_READWRITE);
  bad_spacing_props[PROP_CHILD] = g_param_spec_object ("child", NULL, NULL, G_TYPE_OBJECT, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, N_PROPS, bad_spacing_props);
}

/* Case 2: Very old code using NUM_PROPERTIES */
typedef enum {
  LEGACY_PROP_0,
  LEGACY_PROP_FOO,
  LEGACY_PROP_BAR,
  LEGACY_NUM_PROPERTIES
} LegacyProperty;

static GParamSpec *legacy_props[LEGACY_NUM_PROPERTIES] = { NULL, };

static void
legacy_class_init (LegacyClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  legacy_props[LEGACY_PROP_FOO] = g_param_spec_string ("foo", NULL, NULL, NULL, G_PARAM_READWRITE);
  legacy_props[LEGACY_PROP_BAR] = g_param_spec_string ("bar", NULL, NULL, NULL, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, LEGACY_NUM_PROPERTIES, legacy_props);
}
