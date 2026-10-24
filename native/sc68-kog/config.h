/* Kog's build of sc68's libraries: the defines its autotools configure would
 * set for a plain hosted C build, without curl, audio output or zlib I/O. */
#ifndef KOG_SC68_CONFIG_H
#define KOG_SC68_CONFIG_H
#define PACKAGE_STRING "libsc68 3.0.0 (Kog)"
#define PACKAGE_VERSION "3.0.0"
#define PACKAGE_NAME "libsc68"
#define PACKAGE_TARNAME "libsc68"
#define PACKAGE_URL "http://sc68.atari.org"
#define PACKAGE_BUGREPORT ""
#define VERSION "3.0.0"
#define HAVE_STDINT_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRING_H 1
#define HAVE_LIMITS_H 1
#define HAVE_ASSERT_H 1
#define HAVE_ERRNO_H 1
#define HAVE_CTYPE_H 1
#define HAVE_STDARG_H 1
#define HAVE_STDIO_H 1
#define HAVE_FCNTL_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_SYS_TYPES_H 1
#ifndef _WIN32
# define HAVE_UNISTD_H 1
# define HAVE_FILENO 1
#endif
#define USE_UNICE68 1
#define USE_REPLAY68 1
#define FILE68_UNICE68 1
#endif
