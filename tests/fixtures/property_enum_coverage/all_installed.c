#include <glib-object.h>

typedef struct {
  GObject parent_instance;
} MyObject;

typedef struct {
  GObjectClass parent_class;
} MyObjectClass;

G_DEFINE_TYPE (MyObject, my_object, G_TYPE_OBJECT)

enum {
  PROP_0,
  PROP_FOO,
  PROP_BAR,
  N_PROPS
};

static GParamSpec *properties[N_PROPS];

static void
my_object_class_init (MyObjectClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  properties[PROP_FOO] = g_param_spec_int ("foo", NULL, NULL, 0, 100, 0, G_PARAM_READWRITE);
  properties[PROP_BAR] = g_param_spec_string ("bar", NULL, NULL, NULL, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, N_PROPS, properties);
}

static void
my_object_init (MyObject *self)
{
}
