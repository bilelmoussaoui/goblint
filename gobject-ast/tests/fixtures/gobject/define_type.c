#include <gtk/gtk.h>

struct _MyWidget
{
  GtkWidget parent_instance;
};

G_DEFINE_TYPE (MyWidget, my_widget, GTK_TYPE_WIDGET)

static void
my_widget_init (MyWidget *self)
{
}

static void
my_widget_class_init (MyWidgetClass *klass)
{
}
