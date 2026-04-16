// False positive: if statement contains a for loop, not a g_clear_* call
void
test_for_loop (void)
{
  GList *media_list = NULL;

  // ... some code that may populate media_list ...

cleanup:
  if (media_list) {
    for (; media_list; media_list = g_list_next (media_list)) {
      if (media_list->data) {
        g_free (media_list->data);
      }
    }
  }
}

// Valid: Actually uses g_clear_list, so NULL check is unnecessary
void
test_clear_list (void)
{
  GList *list = NULL;

  // ... some code ...

  if (list) {
    g_clear_list (&list, g_free);
  }
}
