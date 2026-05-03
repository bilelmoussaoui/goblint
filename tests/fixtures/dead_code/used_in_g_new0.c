#include <glib-object.h>

typedef struct {
    guint32 uncompressed_crc;
    guint32 uncompressed_size;
    guint32 compressed_size;
} FuZipFirmwareWriteItem;

static void
foo(GPtrArray *imgs)
{
    g_autofree FuZipFirmwareWriteItem *items = NULL;
    items = g_new0(FuZipFirmwareWriteItem, imgs->len);
    (void)items;
}
