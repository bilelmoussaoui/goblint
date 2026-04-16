#pragma once

#include <glib-object.h>

G_DECLARE_FINAL_TYPE (MyObject, my_object, MY, OBJECT, GObject)

/**
 * my_object_new:
 *
 * Creates a new #MyObject.
 *
 * Returns: (transfer full): a new #MyObject
 */
MyObject *my_object_new (void);

/**
 * my_object_set_name:
 * @self: a #MyObject
 * @name: (nullable): the name to set
 *
 * Sets the name.
 */
void my_object_set_name (MyObject    *self,
                          const char  *name);

/**
 * my_object_get_children:
 * @self: a #MyObject
 *
 * Gets the children.
 *
 * Returns: (transfer none) (element-type MyChild): the children
 */
GList *my_object_get_children (MyObject *self);

/**
 * my_object_process:
 * @self: a #MyObject
 * @callback: (scope async): callback to invoke
 * @user_data: (closure): user data for @callback
 * @destroy: (destroy user_data): destroy notify for @user_data
 *
 * Processes asynchronously.
 */
void my_object_process (MyObject    *self,
                         GCallback    callback,
                         gpointer     user_data,
                         GDestroyNotify destroy);
