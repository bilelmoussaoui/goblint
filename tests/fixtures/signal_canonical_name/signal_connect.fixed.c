// Should trigger: g_signal_connect and variants with underscores
#include <glib-object.h>

void setup_signals(GObject *obj) {
    g_signal_connect(obj, "value-changed", G_CALLBACK(on_value_changed), NULL);
    g_signal_connect_after(obj, "item-selected", G_CALLBACK(on_item_selected), NULL);
    g_signal_connect_swapped(obj, "state-updated", G_CALLBACK(on_state_updated), NULL);
    g_signal_emit_by_name(obj, "notify-user");
}
