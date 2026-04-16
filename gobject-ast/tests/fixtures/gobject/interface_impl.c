#include <glib-object.h>

struct _MyObject
{
  GObject parent_instance;
};

static void my_interface_init (MyInterfaceInterface *iface);

G_DEFINE_TYPE_WITH_CODE (MyObject, my_object, G_TYPE_OBJECT,
                         G_IMPLEMENT_INTERFACE (MY_TYPE_INTERFACE,
                                                my_interface_init))

static void
my_object_do_something (MyInterface *iface)
{
  // Implementation of interface method
}

static void
my_interface_init (MyInterfaceInterface *iface)
{
  iface->do_something = my_object_do_something;
}

static void
my_object_class_init (MyObjectClass *klass)
{
}

static void
my_object_init (MyObject *self)
{
}
