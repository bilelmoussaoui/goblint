#include "config.h"

#include <math.h>

#include <gobject/gvaluecollector.h>
#ifdef HAVE_FONTS
#include <pango/pangocairo.h>
#endif

#include "cogl/cogl.h"

#include "clutter/clutter-actor-private.h"

#ifdef HAVE_FONTS
#include "clutter/pango/clutter-actor-pango.h"
#include "clutter/pango/clutter-pango-private.h"
#endif
#include "clutter/clutter-action.h"
