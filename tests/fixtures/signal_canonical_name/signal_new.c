// Should trigger: g_signal_new with underscores in signal name
#include <glib-object.h>

void my_class_init(GObjectClass *klass) {
    g_signal_new("value_changed",
                 G_TYPE_FROM_CLASS(klass),
                 G_SIGNAL_RUN_LAST,
                 0, NULL, NULL, NULL,
                 G_TYPE_NONE, 1, G_TYPE_INT);

    g_signal_new("item_selected",
                 G_TYPE_FROM_CLASS(klass),
                 G_SIGNAL_RUN_FIRST,
                 0, NULL, NULL, NULL,
                 G_TYPE_NONE, 0);
}
