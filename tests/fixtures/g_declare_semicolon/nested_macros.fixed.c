#include <glib-object.h>

G_DEFINE_TYPE_WITH_CODE (XdpSession, xdp_session, XDP_DBUS_TYPE_SESSION_SKELETON,
                         G_IMPLEMENT_INTERFACE (G_TYPE_INITABLE,
                                                g_initable_iface_init)
                         G_IMPLEMENT_INTERFACE (XDP_DBUS_TYPE_SESSION,
                                                xdp_session_skeleton_iface_init));
