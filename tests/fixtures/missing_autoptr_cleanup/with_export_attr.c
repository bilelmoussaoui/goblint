#include <glib-object.h>

#define META_EXPORT_TEST

META_EXPORT_TEST
G_DECLARE_DERIVABLE_TYPE (MetaWindowX11, meta_window_x11,
                          META, WINDOW_X11, GObject)
