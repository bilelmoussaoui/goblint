#include <glib.h>

typedef struct {
  char *name;
  char *value;
} MyObj;

/* Different variable nulled — not a steal */
static char *
no_steal_different_var (char *a, char *b)
{
  char *result = a;
  b = NULL;
  return result;
}

/* Statement between the two — not consecutive */
static char *
no_steal_gap (char *str)
{
  char *result = str;
  g_print ("%s\n", str);
  str = NULL;
  return result;
}

/* Temp var used in non-return context after NULL */
static void
no_steal_used_after (char *str)
{
  char *result = str;
  str = NULL;
  g_print ("%s\n", result);
  g_free (result);
}

/* Already uses g_steal_pointer */
static char *
already_steal (char *str)
{
  return g_steal_pointer (&str);
}

/* Source is NULL — stealing NULL is pointless */
static char *
no_steal_null_source (void)
{
  char *result = NULL;
  result = NULL;
  return result;
}

/* Wrong order: NULL assignment before copy */
static char *
no_steal_wrong_order (char *str)
{
  str = NULL;
  char *result = str;
  return result;
}

/* 2-statement: right side is NULL, not a pointer to steal */
static void
no_steal_null_rhs (char *str)
{
  char *result = NULL;
  str = NULL;
}

/* Assignment where both sides differ and nulled var doesn't match */
static void
no_steal_mismatch (MyObj *self, char *name)
{
  self->name = name;
  self->value = NULL;
}

/* Dereference expression — g_steal_pointer (&*ptr) is confusing */
static char *
no_steal_deref (char **ptr)
{
  char *result = *ptr;
  *ptr = NULL;
  return result;
}

/* if/else where else assigns a non-NULL default — not a steal */
static void
no_steal_else_default (MyObj *self, char *name)
{
  if (name)
    {
      self->name = name;
      name = NULL;
    }
  else
    {
      self->name = g_strdup ("default");
    }
}

/* if/else where else has more than one statement — not a steal */
static void
no_steal_else_multi (MyObj *self, char *name)
{
  if (name)
    {
      self->name = name;
      name = NULL;
    }
  else
    {
      self->name = NULL;
      self->value = NULL;
    }
}
