#include <glib-object.h>

typedef struct {
    int x;
    int y;
} Point;

// This should NOT be flagged by use_g_autoptr_inline_cleanup
// Plain structs should use g_autofree, not g_autoptr
void test_plain_struct(void)
{
    Point *p = g_new0(Point, 1);
    p->x = 10;
    g_free(p);
}

// This SHOULD be flagged - GObject type
void test_gobject_type(void)
{
    GObject *obj = g_object_new(G_TYPE_OBJECT, NULL);
    g_object_unref(obj);
}
