#include <glib.h>

void test_function(void)
{
  if (1)
    {
      g_autofree char *real_path = NULL;
      g_autofree char *id = NULL;

      id = g_strdup("test");

      if (real_path)
        {
          g_free(real_path);
          real_path = g_strdup("new");
        }
    }
}

char *
test_with_steal_pointer(void)
{
  g_autofree char *result = NULL;

  result = g_strdup("something");

  return g_steal_pointer(&result);
}
