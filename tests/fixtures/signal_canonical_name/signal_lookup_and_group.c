// Should trigger: g_signal_lookup and g_signal_group_connect variants
#include <glib-object.h>

void test_lookup_and_group(GObject *obj, GSignalGroup *group) {
    guint signal_id = g_signal_lookup("value_changed", G_TYPE_FROM_INSTANCE(obj));

    g_signal_group_connect(group, "item_selected", G_CALLBACK(on_item), NULL);
    g_signal_group_connect_after(group, "state_updated", G_CALLBACK(on_state), NULL);
    g_signal_group_connect_object(group, "notify_ready", G_CALLBACK(on_notify), obj, 0);
}
