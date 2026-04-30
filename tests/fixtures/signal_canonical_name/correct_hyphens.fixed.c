// Should NOT trigger: all signal names use hyphens
#include <glib-object.h>

void my_class_init(GObjectClass *klass) {
    g_signal_new("value-changed",
                 G_TYPE_FROM_CLASS(klass),
                 G_SIGNAL_RUN_LAST,
                 0, NULL, NULL, NULL,
                 G_TYPE_NONE, 1, G_TYPE_INT);

    g_signal_new("item-selected",
                 G_TYPE_FROM_CLASS(klass),
                 G_SIGNAL_RUN_FIRST,
                 0, NULL, NULL, NULL,
                 G_TYPE_NONE, 0);
}

void setup_signals(GObject *obj) {
    g_signal_connect(obj, "value-changed", G_CALLBACK(on_value_changed), NULL);
    g_signal_connect_after(obj, "item-selected", G_CALLBACK(on_item_selected), NULL);
    g_signal_emit_by_name(obj, "notify-user");
}
