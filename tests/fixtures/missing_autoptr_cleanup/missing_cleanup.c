typedef struct _MyBoxed MyBoxed;

static MyBoxed *
my_boxed_copy (MyBoxed *boxed)
{
  return boxed;
}

static void
my_boxed_free (MyBoxed *boxed)
{
  (void)boxed;
}

G_DEFINE_BOXED_TYPE (MyBoxed, my_boxed, my_boxed_copy, my_boxed_free)
