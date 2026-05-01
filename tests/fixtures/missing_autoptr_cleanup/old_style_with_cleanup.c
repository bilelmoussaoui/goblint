#include <glib-object.h>

typedef struct _MyOldStyleObject MyOldStyleObject;
typedef struct _MyOldStyleObjectClass MyOldStyleObjectClass;

struct _MyOldStyleObject
{
  GObject parent_instance;
};

struct _MyOldStyleObjectClass
{
  GObjectClass parent_class;
};

GType my_old_style_object_get_type (void);

G_DEFINE_TYPE (MyOldStyleObject, my_old_style_object, G_TYPE_OBJECT)

G_DEFINE_AUTOPTR_CLEANUP_FUNC (MyOldStyleObject, g_object_unref)

static void
my_old_style_object_class_init (MyOldStyleObjectClass *klass)
{
  (void)klass;
}

static void
my_old_style_object_init (MyOldStyleObject *self)
{
  (void)self;
}
