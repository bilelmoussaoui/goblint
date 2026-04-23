// Should trigger violation: hex power-of-two flags
typedef enum
{
  PERMISSION_NONE = 0x00,
  PERMISSION_READ = 0x01,
  PERMISSION_WRITE = 0x02,
  PERMISSION_EXECUTE = 0x04,
  PERMISSION_DELETE = 0x08
} PermissionFlags;
