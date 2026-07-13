/* c-auto.h - hand-synthesized configuration for *-pc-windows-msvc.
   Stands in for kpathsea's autoconf-generated header (there is no
   ./configure step in the vendored `cc` build). Values reflect the MSVC
   (cl.exe / UCRT) feature set and are fixed for the target, so this is
   maintained by hand rather than probed. Revisit on a vendored-kpathsea
   version bump. See vendor/README.md for provenance. */
#ifndef KPATHSEA_C_AUTO_H
#define KPATHSEA_C_AUTO_H

/* Headers MSVC ships. */
#define HAVE_ASSERT_H 1
#define HAVE_FLOAT_H 1
#define HAVE_INTTYPES_H 1
#define HAVE_LIMITS_H 1
#define HAVE_STDINT_H 1
#define HAVE_STDIO_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRING_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_SYS_TYPES_H 1
#define HAVE_WCHAR_H 1
#define STDC_HEADERS 1

/* Headers MSVC lacks (left undefined): DIRENT_H, DLFCN_H, MINIX_CONFIG_H,
   NDIR_H, PWD_H, STRINGS_H, SYS_DIR_H, SYS_NDIR_H, SYS_PARAM_H, UNISTD_H.
   win32lib.h supplies the WIN32 dirent/opendir shims instead. */

/* Functions. win32lib.h remaps getcwd/putenv/mktemp/stat to the _-prefixed
   MSVC spellings, so from kpathsea's view these are present. */
#define HAVE_GETCWD 1
#define HAVE_PUTENV 1
#define HAVE_MKTEMP 1
#define HAVE_MEMCMP 1
#define HAVE_MEMCPY 1
#define HAVE_STRCHR 1
#define HAVE_STRRCHR 1
#define HAVE_DECL_ISASCII 1
#define HAVE_DECL_PUTENV 1
/* Absent on MSVC (left undefined): HAVE_FSEEKO, HAVE_MKSTEMP,
   HAVE_STRUCT_STAT_ST_MTIM. */

/* Generation of missing files: OFF (lookup-only library). Lets us drop the
   win32/ mktex* subdir entirely. */
#define MAKE_TEX_FMT_BY_DEFAULT 0
#define MAKE_TEX_MF_BY_DEFAULT 0
#define MAKE_TEX_PK_BY_DEFAULT 0
#define MAKE_TEX_TEX_BY_DEFAULT 0
#define MAKE_TEX_TFM_BY_DEFAULT 0
#define MAKE_OMEGA_OCP_BY_DEFAULT 0
#define MAKE_OMEGA_OFM_BY_DEFAULT 0

/* Sizes / types. Windows is LLP64: long is 32-bit. */
#define SIZEOF_LONG 4

/* Package identity. */
#define PACKAGE "kpathsea"
#define PACKAGE_NAME "Kpathsea"
#define PACKAGE_TARNAME "kpathsea"
#define PACKAGE_VERSION "6.4.3/dev"
#define PACKAGE_STRING "Kpathsea 6.4.3/dev"
#define PACKAGE_BUGREPORT "tex-k@tug.org"
#define PACKAGE_URL ""
#define VERSION "6.4.3/dev"
#define KPSEVERSION "kpathsea version 6.4.3/dev"

/* MSVC's snprintf is C99-conformant (VS2015+); config.h still wraps on WIN32. */

#endif /* KPATHSEA_C_AUTO_H */
