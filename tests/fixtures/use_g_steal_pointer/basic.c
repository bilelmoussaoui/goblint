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
  char *result = str;
  str = NULL;
  return result;
}

static char *
steal_member_field (MyObj *self)
{
  char *name = self->name;
  self->name = NULL;
  return name;
}

static GList *
steal_list_field (MyObj *self)
{
  GList *items = self->items;
  self->items = NULL;
  return items;
}

static gpointer
steal_gpointer (gpointer ptr)
{
  gpointer result = ptr;
  ptr = NULL;
  return result;
}

/* 3-statement pattern inside an if block */

static char *
steal_in_branch (MyObj *self, gboolean condition)
{
  if (condition)
    {
      char *name = self->name;
      self->name = NULL;
      return name;
    }

  return NULL;
}

/* 2-statement pattern: other = ptr; ptr = NULL; */

static void
steal_param_to_member (MyObj *self, char *name)
{
  g_free (self->name);
  self->name = name;
  name = NULL;
}

static void
steal_member_to_member (MyObj *dst, MyObj *src)
{
  g_free (dst->name);
  dst->name = src->name;
  src->name = NULL;
}

static void
steal_local_to_local (char **dest, char *src)
{
  *dest = src;
  src = NULL;
}

static void
steal_nested_member (MyObj *self, char *value)
{
  g_free (self->value);
  self->value = value;
  value = NULL;
}

/* if without else — braces removed, 2-stmt assign pattern */

static void
steal_if_no_else (MyObj *self, char *name)
{
  if (name)
    {
      self->name = name;
      name = NULL;
    }
}

/* if/else steal: if (expr) { dest = expr; expr = NULL; } else { dest = NULL; } */

static void
steal_if_else_simple (MyObj *self, char *name)
{
  g_free (self->name);
  if (name)
    {
      self->name = name;
      name = NULL;
    }
  else
    {
      self->name = NULL;
    }
}

static void
steal_if_else_explicit_null (MyObj *self, char *value)
{
  g_free (self->value);
  if (value != NULL)
    {
      self->value = value;
      value = NULL;
    }
  else
    {
      self->value = NULL;
    }
}
