#include <glib.h>

typedef struct {
    int x;
    int y;
} Point;

typedef struct {
    char *name;
    int age;
} Person;

void test_plain_struct(void)
{
    // Should suggest g_autofree - plain struct allocated with g_new0
    Point *p = g_new0(Point, 1);
    p->x = 10;
    p->y = 20;
    g_free(p);

    // Should suggest g_autofree - plain struct allocated with g_malloc
    Person *person = g_malloc(sizeof(Person));
    person->age = 25;
    g_free(person);
}
