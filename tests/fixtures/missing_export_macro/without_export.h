#pragma once

#include <glib-object.h>

#define MY_TYPE_WIDGET (my_widget_get_type ())
G_DECLARE_FINAL_TYPE (MyWidget, my_widget, MY, WIDGET, GObject)

void my_widget_show (MyWidget *self);
