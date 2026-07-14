/* c-auto.h - hand-written config for the from-source build on Unix.
   Stands in for kpathsea's autoconf-generated header (build.rs compiles the
   sources directly, with no ./configure step). Encodes the POSIX/glibc feature
   set; verified on Linux, best-effort on other Unix. Revisit on a KPSE_REF bump.
   See common/README.md. */
#ifndef KPATHSEA_C_AUTO_H
#define KPATHSEA_C_AUTO_H

/* Headers a POSIX libc provides. */
#define HAVE_ASSERT_H 1
#define HAVE_DIRENT_H 1
#define HAVE_DLFCN_H 1
#define HAVE_FLOAT_H 1
#define HAVE_INTTYPES_H 1
#define HAVE_LIMITS_H 1
#define HAVE_PWD_H 1
#define HAVE_STDINT_H 1
#define HAVE_STDIO_H 1
#define HAVE_STDLIB_H 1
#define HAVE_STRINGS_H 1
#define HAVE_STRING_H 1
#define HAVE_SYS_PARAM_H 1
#define HAVE_SYS_STAT_H 1
#define HAVE_SYS_TYPES_H 1
#define HAVE_UNISTD_H 1
#define HAVE_WCHAR_H 1
#define STDC_HEADERS 1

/* Functions / declarations (build.rs passes -D_GNU_SOURCE, so the glibc
   extensions below are visible). */
#define HAVE_GETCWD 1
#define HAVE_PUTENV 1
#define HAVE_MKTEMP 1
#define HAVE_MKSTEMP 1
#define HAVE_MEMCMP 1
#define HAVE_MEMCPY 1
#define HAVE_STRCHR 1
#define HAVE_STRRCHR 1
#define HAVE_FSEEKO 1
#define HAVE_STRUCT_STAT_ST_MTIM 1
#define HAVE_DECL_ISASCII 1
#define HAVE_DECL_PUTENV 1

/* Generation of missing files: OFF (lookup-only library). */
#define MAKE_TEX_FMT_BY_DEFAULT 0
#define MAKE_TEX_MF_BY_DEFAULT 0
#define MAKE_TEX_PK_BY_DEFAULT 0
#define MAKE_TEX_TEX_BY_DEFAULT 0
#define MAKE_TEX_TFM_BY_DEFAULT 0
#define MAKE_OMEGA_OCP_BY_DEFAULT 0
#define MAKE_OMEGA_OFM_BY_DEFAULT 0

/* sizeof(long): 8 on LP64 (64-bit Linux/macOS), 4 on ILP32. */
#if defined(__LP64__) || defined(_LP64)
#define SIZEOF_LONG 8
#else
#define SIZEOF_LONG 4
#endif

/* Package identity. */
#define PACKAGE "kpathsea"
#define PACKAGE_NAME "Kpathsea"
#define PACKAGE_TARNAME "kpathsea"
#define PACKAGE_VERSION "6.4.1"
#define PACKAGE_STRING "Kpathsea 6.4.1"
#define PACKAGE_BUGREPORT "tex-k@tug.org"
#define PACKAGE_URL ""
#define VERSION "6.4.1"
#define KPSEVERSION "kpathsea version 6.4.1"

#endif /* KPATHSEA_C_AUTO_H */
