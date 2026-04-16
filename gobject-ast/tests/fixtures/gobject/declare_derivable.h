#pragma once

#include <glib-object.h>

G_DECLARE_DERIVABLE_TYPE (MyObject, my_object, MY, OBJECT, GObject)

struct _MyObjectClass
{
  GObjectClass parent_class;
};
