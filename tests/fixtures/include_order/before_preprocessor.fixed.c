#include "config.h"

#include <string.h>

#include <X11/Xatom.h>
#include <unistd.h>

#include "compositor/compositor-private.h"
#include "core/meta-window-config-private.h"
#include "x11/window-props.h"
#include "x11/xprops.h"

#ifndef HOST_NAME_MAX
#define HOST_NAME_MAX 255
#endif
