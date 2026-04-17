#include "config.h"

#include <glib.h>
#include <stdio.h>

#include "foo.h"

#ifdef HAVE_X11
#include <X11/Xlib.h>
#include "platform-x11.h"
#include <X11/Xatom.h>
#endif

#include "bar.h"

void test(void) {
}
