typedef struct _MyBoxedWithCode MyBoxedWithCode;

static MyBoxedWithCode *
my_boxed_with_code_copy (MyBoxedWithCode *boxed)
{
  return boxed;
}

static void
my_boxed_with_code_free (MyBoxedWithCode *boxed)
{
  (void)boxed;
}

G_DEFINE_BOXED_TYPE_WITH_CODE (MyBoxedWithCode,
                                my_boxed_with_code,
                                my_boxed_with_code_copy,
                                my_boxed_with_code_free,
                                /* no code */)
