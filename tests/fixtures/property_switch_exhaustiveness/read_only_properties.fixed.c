#include <glib-object.h>

typedef struct {
  GObject parent_instance;
  char *name;
  int status;
} MyObject;

typedef struct {
  GObjectClass parent_class;
} MyObjectClass;

G_DEFINE_TYPE (MyObject, my_object, G_TYPE_OBJECT)

enum {
  PROP_NAME = 1,
  PROP_STATUS,
};

static GParamSpec *props[PROP_STATUS + 1] = { NULL, };

static void
my_object_get_property (GObject    *object,
                        guint       prop_id,
                        GValue     *value,
                        GParamSpec *pspec)
{
  MyObject *self = MY_OBJECT (object);

  switch (prop_id)
    {
    case PROP_NAME:
      g_value_set_string (value, self->name);
      break;
    /* PROP_STATUS is missing - but it's read-only, should still be here */
    default:
      G_OBJECT_WARN_INVALID_PROPERTY_ID (object, prop_id, pspec);
      break;
    }
}

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
    case PROP_STATUS:
      g_assert_not_reached ();
      break;
    default:
      G_OBJECT_WARN_INVALID_PROPERTY_ID (object, prop_id, pspec);
      break;
    }
}

static void
my_object_class_init (MyObjectClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  object_class->get_property = my_object_get_property;
  object_class->set_property = my_object_set_property;

  props[PROP_NAME] = g_param_spec_string ("name", NULL, NULL, NULL, G_PARAM_READWRITE);
  props[PROP_STATUS] = g_param_spec_int ("status", NULL, NULL, 0, 100, 0, G_PARAM_READABLE);

  g_object_class_install_properties (object_class, G_N_ELEMENTS (props), props);
}

static void
my_object_init (MyObject *self)
{
}
