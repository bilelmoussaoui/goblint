/* Static function that IS called — no violation */
static void
helper (void)
{
}

void
public_entry_point (void)
{
  helper ();
}
