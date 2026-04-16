#include <glib-object.h>

typedef struct _MyStruct MyStruct;

struct _MyStruct
{
  int x;
  int y;
  char *name;
};

static MyStruct *
my_struct_copy (MyStruct *src)
{
  MyStruct *dest = g_new0 (MyStruct, 1);
  dest->x = src->x;
  dest->y = src->y;
  dest->name = g_strdup (src->name);
  return dest;
}

static void
my_struct_free (MyStruct *self)
{
  g_free (self->name);
  g_free (self);
}

G_DEFINE_BOXED_TYPE (MyStruct, my_struct, my_struct_copy, my_struct_free)

typedef struct _MyOpaqueStruct MyOpaqueStruct;

G_DEFINE_POINTER_TYPE (MyOpaqueStruct, my_opaque_struct)
