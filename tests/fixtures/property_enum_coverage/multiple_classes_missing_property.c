#include <glib-object.h>

/* First class: MyItem */
typedef struct {
  GObject parent_instance;
  char *title;
  int priority;
} MyItem;

typedef struct {
  GObjectClass parent_class;
} MyItemClass;

G_DEFINE_TYPE (MyItem, my_item, G_TYPE_OBJECT)

enum {
  ITEM_PROP_TITLE = 1,
  ITEM_PROP_PRIORITY
};

static GParamSpec *item_props[ITEM_PROP_PRIORITY + 1] = { NULL, };

static void
my_item_class_init (MyItemClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  item_props[ITEM_PROP_TITLE] = g_param_spec_string ("title", NULL, NULL, NULL, G_PARAM_READWRITE);
  /* Missing: ITEM_PROP_PRIORITY */

  g_object_class_install_properties (object_class, G_N_ELEMENTS (item_props), item_props);
}

static void
my_item_init (MyItem *self)
{
}

/* Second class: MyContainer */
typedef struct {
  GObject parent_instance;
  char *name;
} MyContainer;

typedef struct {
  GObjectClass parent_class;
} MyContainerClass;

G_DEFINE_TYPE (MyContainer, my_container, G_TYPE_OBJECT)

enum {
  CONTAINER_PROP_NAME = 1,
};

static GParamSpec *container_props[CONTAINER_PROP_NAME + 1] = { NULL, };

static void
my_container_class_init (MyContainerClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  container_props[CONTAINER_PROP_NAME] = g_param_spec_string ("name", NULL, NULL, NULL, G_PARAM_READWRITE);

  g_object_class_install_properties (object_class, G_N_ELEMENTS (container_props), container_props);
}

static void
my_container_init (MyContainer *self)
{
}
