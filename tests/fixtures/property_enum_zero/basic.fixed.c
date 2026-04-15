#include <glib-object.h>

/* First enum: missing PROP_0, has = 0 */
typedef enum {
  PROP_0,
  PROP_NAME,
  PROP_TITLE,
  PROP_DESCRIPTION,
  N_PROPS
} MyObjectProps;

/* Second enum: missing PROP_0 - will conflict with first when fixed */
typedef enum {
  MY_OBJECT_PROPS2_PROP_0,
  PROP_FOO,
  PROP_BAR,
  MY_OBJECT_PROPS2_N_PROPS
} MyObjectProps2;

/* Third enum: already correct with prefix */
typedef enum {
  WIDGET_PROPS_PROP_0,
  PROP_WIDTH,
  PROP_HEIGHT,
  WIDGET_PROPS_N_PROPS
} WidgetProps;
