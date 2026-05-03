#pragma once

#include <glib-object.h>

#define MY_TYPE_OBJECT (my_object_get_type ())
MY_EXPORT
G_DECLARE_FINAL_TYPE (MyObject, my_object, MY, OBJECT, GObject)

MY_EXPORT
void my_object_do_something (MyObject *self);
