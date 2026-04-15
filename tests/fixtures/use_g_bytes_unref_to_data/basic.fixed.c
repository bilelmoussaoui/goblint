#include <glib.h>

/* BAD: g_bytes_get_data followed by g_bytes_unref */
static gconstpointer
test_bytes_bad_1 (GBytes *bytes)
{
  gsize size;
  gconstpointer data;

  data = g_bytes_unref_to_data (bytes, &size);

  return data;
}

static const char *
test_bytes_bad_2 (GBytes *bytes, gsize *out_size)
{
  const char *result;

  result = g_bytes_unref_to_data (bytes, out_size);

  return result;
}

static void
test_bytes_bad_in_branch (GBytes *bytes, gboolean condition)
{
  if (condition)
    {
      gsize size;
      const void *data;

      data = g_bytes_unref_to_data (bytes, &size);

      g_print ("Data: %p\n", data);
    }
}

/* GOOD: Already using g_bytes_unref_to_data */
static gconstpointer
test_bytes_good_1 (GBytes *bytes)
{
  gsize size;

  return g_bytes_unref_to_data (bytes, &size);
}

/* GOOD: g_bytes_unref without preceding g_bytes_get_data */
static void
test_bytes_good_2 (GBytes *bytes)
{
  g_bytes_unref (bytes);
}

/* GOOD: g_bytes_get_data without g_bytes_unref */
static gconstpointer
test_bytes_good_3 (GBytes *bytes)
{
  gsize size;

  return g_bytes_get_data (bytes, &size);
}

/* GOOD: Different bytes variable */
static void
test_bytes_good_4 (GBytes *bytes1, GBytes *bytes2)
{
  gsize size;
  gconstpointer data;

  data = g_bytes_get_data (bytes1, &size);
  g_bytes_unref (bytes2);  /* Different variable */
}
