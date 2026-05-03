#pragma once

#include <glib-object.h>

/* Private type — no export macro required even though it lacks one */
#define MY_TYPE_INTERNAL (my_internal_get_type ())
G_DECLARE_FINAL_TYPE (MyInternal, my_internal, MY, INTERNAL, GObject)

void my_internal_helper (MyInternal *self);
