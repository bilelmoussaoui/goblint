#include <glib-object.h>

struct _MyObject
{
  GObject parent_instance;
  char *name;
  int value;
};

G_DEFINE_TYPE (MyObject, my_object, G_TYPE_OBJECT)

enum {
  PROP_0,
  PROP_NAME,
  PROP_VALUE,
  N_PROPS
};

static GParamSpec *properties[N_PROPS];

static void
my_object_set_property (GObject      *object,
                         guint         prop_id,
                         const GValue *value,
                         GParamSpec   *pspec)
{
  MyObject *self = MY_OBJECT (object);

  switch (prop_id)
    {
    case PROP_NAME:
      g_free (self->name);
      self->name = g_value_dup_string (value);
      break;
    case PROP_VALUE:
      self->value = g_value_get_int (value);
      break;
    default:
      G_OBJECT_WARN_INVALID_PROPERTY_ID (object, prop_id, pspec);
    }
}

static void
my_object_class_init (MyObjectClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  object_class->set_property = my_object_set_property;

  properties[PROP_NAME] =
    g_param_spec_string ("name",
                         "Name",
                         "The object name",
                         NULL,
                         G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS);

  properties[PROP_VALUE] =
    g_param_spec_int ("value",
                      "Value",
                      "The object value",
                      0, 100, 0,
                      G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS);

  g_object_class_install_properties (object_class, N_PROPS, properties);
}

static void
my_object_init (MyObject *self)
{
}
