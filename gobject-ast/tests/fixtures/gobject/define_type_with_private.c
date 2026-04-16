#include <glib-object.h>

typedef struct
{
  int counter;
} MyObjectPrivate;

struct _MyObject
{
  GObject parent_instance;
};

G_DEFINE_TYPE_WITH_PRIVATE (MyObject, my_object, G_TYPE_OBJECT)

static void
my_object_init (MyObject *self)
{
}

static void
my_object_class_init (MyObjectClass *klass)
{
}
