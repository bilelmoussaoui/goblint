#include <glib-object.h>

typedef struct _FooObject FooObject;

G_DEFINE_TYPE_EXTENDED(FooObject, foo_object, G_TYPE_OBJECT, 0,
                       G_IMPLEMENT_INTERFACE(FOO_TYPE_CODEC, foo_object_codec_iface_init))

static void
foo_object_codec_iface_init(FooCodecInterface *iface)
{
    iface->encode = foo_object_encode;
}

static void
foo_object_init(FooObject *self)
{
}

static void
foo_object_class_init(FooObjectClass *klass)
{
}
