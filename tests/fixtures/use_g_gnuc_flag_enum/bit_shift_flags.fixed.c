// Should trigger violation: flags enum without G_GNUC_FLAG_ENUM
typedef enum
{
  INTEROP_FLAGS_EXTERNAL = (1 << 0),
  INTEROP_FLAGS_ANONYMOUS = (1 << 1),
  INTEROP_FLAGS_SHA1 = (1 << 2),
  INTEROP_FLAGS_TCP = (1 << 3),
  INTEROP_FLAGS_LIBDBUS = (1 << 4),
  INTEROP_FLAGS_ABSTRACT = (1 << 5),
  INTEROP_FLAGS_REQUIRE_SAME_USER = (1 << 6),
  INTEROP_FLAGS_NONE = 0
} G_GNUC_FLAG_ENUM InteropFlags;
