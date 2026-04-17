#include <adwaita.h>

void test_empty_strings(void)
{
    AdwActionRow *row;

    // Should NOT be flagged - empty string doesn't need translation
    adw_action_row_set_subtitle(ADW_ACTION_ROW(row), "");

    // Should NOT be flagged - empty string
    adw_action_row_set_title(ADW_ACTION_ROW(row), "");

    // Should be flagged - non-empty string
    adw_action_row_set_subtitle(ADW_ACTION_ROW(row), "Details");
}
