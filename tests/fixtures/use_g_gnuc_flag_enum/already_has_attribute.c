// Should pass: already has G_GNUC_FLAG_ENUM
typedef enum
{
  MY_FLAGS_NONE = 0,
  MY_FLAGS_READ = (1 << 0),
  MY_FLAGS_WRITE = (1 << 1),
  MY_FLAGS_EXECUTE = (1 << 2)
} G_GNUC_FLAG_ENUM MyFlags;
