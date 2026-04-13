#include <glib.h>

typedef struct {
  char  *name;
  char  *value;
  GList *items;
} MyObj;

/* 3-statement pattern: T *tmp = ptr; ptr = NULL; return tmp; */

static char *
steal_local_param (char *str)
{
  return g_steal_pointer (&str);
}

static char *
steal_member_field (MyObj *self)
{
  return g_steal_pointer (&self->name);
}

static GList *
steal_list_field (MyObj *self)
{
  return g_steal_pointer (&self->items);
}

static gpointer
steal_gpointer (gpointer ptr)
{
  return g_steal_pointer (&ptr);
}

/* 3-statement pattern inside an if block */

static char *
steal_in_branch (MyObj *self, gboolean condition)
{
  if (condition)
    return g_steal_pointer (&self->name);

  return NULL;
}

/* 2-statement pattern: other = ptr; ptr = NULL; */

static void
steal_param_to_member (MyObj *self, char *name)
{
  g_free (self->name);
  self->name = g_steal_pointer (&name);
}

static void
steal_member_to_member (MyObj *dst, MyObj *src)
{
  g_free (dst->name);
  dst->name = g_steal_pointer (&src->name);
}

static void
steal_local_to_local (char **dest, char *src)
{
  *dest = g_steal_pointer (&src);
}

static void
steal_nested_member (MyObj *self, char *value)
{
  g_free (self->value);
  self->value = g_steal_pointer (&value);
}

/* if without else — braces removed, 2-stmt assign pattern */

static void
steal_if_no_else (MyObj *self, char *name)
{
  if (name)
    self->name = g_steal_pointer (&name);
}

/* if/else steal: if (expr) { dest = expr; expr = NULL; } else { dest = NULL; } */

static void
steal_if_else_simple (MyObj *self, char *name)
{
  g_free (self->name);
  self->name = g_steal_pointer (&name);
}

static void
steal_if_else_explicit_null (MyObj *self, char *value)
{
  g_free (self->value);
  self->value = g_steal_pointer (&value);
}
