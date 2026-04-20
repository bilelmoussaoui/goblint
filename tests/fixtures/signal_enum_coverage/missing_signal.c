#include <glib-object.h>

typedef struct {
  GObject parent_instance;
} MyButton;

typedef struct {
  GObjectClass parent_class;
} MyButtonClass;

G_DEFINE_TYPE (MyButton, my_button, G_TYPE_OBJECT)

enum {
  SIGNAL_CLICKED,
  SIGNAL_ACTIVATED,
  SIGNAL_RELEASED,  // Missing installation!
  N_SIGNALS
};

static guint signals[N_SIGNALS];

static void
my_button_class_init (MyButtonClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);

  signals[SIGNAL_CLICKED] = g_signal_new ("clicked",
                                           G_TYPE_FROM_CLASS (klass),
                                           G_SIGNAL_RUN_LAST,
                                           0,
                                           NULL, NULL, NULL,
                                           G_TYPE_NONE, 0);

  signals[SIGNAL_ACTIVATED] = g_signal_new ("activated",
                                             G_TYPE_FROM_CLASS (klass),
                                             G_SIGNAL_RUN_LAST,
                                             0,
                                             NULL, NULL, NULL,
                                             G_TYPE_NONE, 0);
  // SIGNAL_RELEASED is missing!
}

static void
my_button_init (MyButton *self)
{
}
