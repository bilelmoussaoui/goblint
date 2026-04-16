#include <glib-object.h>

struct _MyObject
{
  GObject parent_instance;
};

G_DEFINE_TYPE (MyObject, my_object, G_TYPE_OBJECT)

enum {
  SIGNAL_CHANGED,
  SIGNAL_ACTIVATED,
  N_SIGNALS
};

static guint signals[N_SIGNALS];

static void
my_object_changed_default (MyObject *self)
{
  // Default implementation
}

static void
my_object_class_init (MyObjectClass *klass)
{
  klass->changed = my_object_changed_default;

  signals[SIGNAL_CHANGED] =
    g_signal_new ("changed",
                  G_TYPE_FROM_CLASS (klass),
                  G_SIGNAL_RUN_LAST,
                  G_STRUCT_OFFSET (MyObjectClass, changed),
                  NULL, NULL,
                  NULL,
                  G_TYPE_NONE,
                  0);

  signals[SIGNAL_ACTIVATED] =
    g_signal_new ("activated",
                  G_TYPE_FROM_CLASS (klass),
                  G_SIGNAL_RUN_FIRST,
                  0,
                  NULL, NULL,
                  NULL,
                  G_TYPE_NONE,
                  1,
                  G_TYPE_INT);
}

static void
my_object_init (MyObject *self)
{
}
