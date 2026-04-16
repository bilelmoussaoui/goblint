#pragma once

#include <gtk/gtk.h>

G_DECLARE_FINAL_TYPE (MyWidget, my_widget, MY, WIDGET, GtkWidget)

G_DECLARE_INTERFACE (MyInterface, my_interface, MY, INTERFACE, GObject)
