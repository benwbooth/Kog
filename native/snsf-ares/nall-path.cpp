// Extracted unchanged from pinned ISC nall/path.cpp by vendor.py.
#include <nall/path.hpp>
#if defined(PLATFORM_WINDOWS)
#include <windows.h>
#endif
namespace nall::Path {
NALL_HEADER_INLINE auto temporary() -> string {
  #if defined(PLATFORM_WINDOWS)
  wchar_t path[PATH_MAX] = L"";
  GetTempPathW(PATH_MAX, path);
  string result = (const char*)utf8_t(path);
  result.transform("\\", "/");
  #elif defined(P_tmpdir)
  string result = P_tmpdir;
  #else
  string result = "/tmp/";
  #endif
  if(!result.endsWith("/")) result.append("/");
  return result;
}

}
