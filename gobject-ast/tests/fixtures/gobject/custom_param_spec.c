#include <glib-object.h>

typedef struct _MyObject MyObject;
typedef struct _MyObjectClass MyObjectClass;

struct _MyObject
{
  GObject parent_instance;
};

struct _MyObjectClass
{
  GObjectClass parent_class;
};

G_DEFINE_TYPE (MyObject, my_object, G_TYPE_OBJECT)

// Custom param spec function (like from Cogl, Clutter, etc.)
GParamSpec *
cogl_param_spec_color (const gchar *name,
                       const gchar *nick,
                       const gchar *blurb,
                       const void *default_value,
                       GParamFlags flags)
{
  return NULL; // stub
}

enum
{
  PROP_0,
  PROP_COLOR,
  N_PROPS
};

static GParamSpec *properties[N_PROPS];

static void
my_object_class_init (MyObjectClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  properties[PROP_COLOR] =
      cogl_param_spec_color ("color",
                             "Color",
                             "The object color",
                             NULL,
                             G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS);

  g_object_class_install_properties (object_class, N_PROPS, properties);
}

static void
my_object_init (MyObject *self)
{
}
