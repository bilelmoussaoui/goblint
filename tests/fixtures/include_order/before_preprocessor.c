#include "config.h"

#include "x11/window-props.h"

#include <X11/Xatom.h>
#include <unistd.h>
#include <string.h>

#include "compositor/compositor-private.h"
#include "core/meta-window-config-private.h"
#include "x11/xprops.h"

#ifndef HOST_NAME_MAX
#define HOST_NAME_MAX 255
#endif
