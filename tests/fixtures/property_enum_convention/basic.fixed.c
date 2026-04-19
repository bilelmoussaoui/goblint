#include <glib-object.h>

/* Case 1: Old pattern with PROP_0 and N_PROPS */
typedef enum {
  PROP_NAME = 1,
  PROP_TITLE,
  PROP_DESCRIPTION,
} MyObjectProperty;

static GParamSpec *my_props[PROP_DESCRIPTION + 1] = { NULL, };

static void
my_object_class_init (MyObjectClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  my_props[PROP_NAME] = g_param_spec_string ("name", NULL, NULL, NULL, G_PARAM_READWRITE);
  my_props[PROP_TITLE] = g_param_spec_string ("title", NULL, NULL, NULL, G_PARAM_READWRITE);
  my_props[PROP_DESCRIPTION] = g_param_spec_string ("description", NULL, NULL, NULL, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, G_N_ELEMENTS (my_props), my_props);
}

/* Case 2: Old pattern with prefix */
typedef enum {
  WIDGET_PROP_WIDTH = 1,
  WIDGET_PROP_HEIGHT,
} WidgetProperty;

static GParamSpec *widget_props[WIDGET_PROP_HEIGHT + 1] = { NULL, };

static void
widget_class_init (WidgetClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  widget_props[WIDGET_PROP_WIDTH] = g_param_spec_int ("width", NULL, NULL, 0, 100, 0, G_PARAM_READWRITE);
  widget_props[WIDGET_PROP_HEIGHT] = g_param_spec_int ("height", NULL, NULL, 0, 100, 0, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, G_N_ELEMENTS (widget_props), widget_props);
}

/* Case 3: Already using modern pattern - should NOT be flagged */
typedef enum {
  MODERN_PROP_FOO = 1,
  MODERN_PROP_BAR,
  MODERN_PROP_BAZ
} ModernProperty;

static GParamSpec *modern_props[MODERN_PROP_BAZ + 1] = { NULL, };

static void
modern_class_init (ModernClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  modern_props[MODERN_PROP_FOO] = g_param_spec_string ("foo", NULL, NULL, NULL, G_PARAM_READWRITE);
  modern_props[MODERN_PROP_BAR] = g_param_spec_string ("bar", NULL, NULL, NULL, G_PARAM_READWRITE);
  modern_props[MODERN_PROP_BAZ] = g_param_spec_string ("baz", NULL, NULL, NULL, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, G_N_ELEMENTS (modern_props), modern_props);
}

/* Case 4: Non-GParamSpec array using N_PROPS - should NOT be touched */
static int counts[N_PROPS];

/* Case 5: GParamSpec array in conditional block */
#ifdef ENABLE_FEATURE
typedef enum {
  FEATURE_PROP_ENABLED = 1,
  FEATURE_PROP_VALUE,
} FeatureProperty;

static GParamSpec *feature_props[FEATURE_PROP_VALUE + 1] = { NULL, };

static void
feature_class_init (FeatureClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  feature_props[FEATURE_PROP_ENABLED] = g_param_spec_boolean ("enabled", NULL, NULL, FALSE, G_PARAM_READWRITE);
  feature_props[FEATURE_PROP_VALUE] = g_param_spec_int ("value", NULL, NULL, 0, 100, 0, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, G_N_ELEMENTS (feature_props), feature_props);
}
#endif

/* Case 6: install_properties call that doesn't use the right array - should NOT be touched */
static void
wrong_usage (void)
{
  GObjectClass *object_class = NULL;

  /* This uses N_PROPS but passes a different array that we don't track */
  g_object_class_install_properties (object_class, N_PROPS, some_other_props);
}
