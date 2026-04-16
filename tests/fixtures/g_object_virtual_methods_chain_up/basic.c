#include <glib-object.h>

static void
foo_dispose (GObject *object)
{
  FooPrivate *priv = foo_get_instance_private (FOO (object));
  g_clear_object (&priv->child);
}

// Valid: chains up using object_class variable
static void
bar_dispose (GObject *object)
{
  BarPrivate *priv = bar_get_instance_private (BAR (object));
  GObjectClass *object_class = G_OBJECT_CLASS (bar_parent_class);

  g_clear_object (&priv->child);

  object_class->dispose (object);
}

// Valid: chains up using klass variable
static void
baz_finalize (GObject *object)
{
  GObjectClass *klass = G_OBJECT_CLASS (baz_parent_class);

  // Some cleanup
  g_free (object->data);

  klass->finalize (object);
}

// Valid: Not a GObject virtual method - GSource has its own finalize
static void
callback_source_finalize (GSource *source)
{
  CallbackSource *callback_source = (CallbackSource *) source;
  g_clear_pointer (&callback_source->callback, g_free);
}

// Valid: Chains up correctly (from real gnome-shell code)
static void
meta_virtual_input_device_native_dispose (GObject *object)
{
  ClutterVirtualInputDevice *virtual_device =
    CLUTTER_VIRTUAL_INPUT_DEVICE (object);
  MetaVirtualInputDeviceNative *virtual_native =
    META_VIRTUAL_INPUT_DEVICE_NATIVE (object);
  MetaSeatNative *seat_native =
    meta_virtual_input_device_native_get_seat_native (virtual_native);
  GObjectClass *object_class =
    G_OBJECT_CLASS (meta_virtual_input_device_native_parent_class);

  if (virtual_native->impl_state)
    {
      GTask *task;

      task = g_task_new (virtual_device, NULL, NULL, NULL);
      g_task_set_task_data (task, virtual_native->impl_state,
                            (GDestroyNotify) impl_state_free);
      meta_seat_impl_run_input_task (seat_native->impl, task,
                                     (GSourceFunc) release_device_in_impl);
      g_object_unref (task);

      virtual_native->impl_state = NULL;
    }

  meta_seat_native_release_touch_slots (seat_native,
                                        virtual_native->slot_base);

  object_class->dispose (object);
}
