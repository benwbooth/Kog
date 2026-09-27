cmake_minimum_required(VERSION 3.16)
project(kog_archive_deps LANGUAGES C)
set(CMAKE_POLICY_VERSION_MINIMUM 3.5)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
get_filename_component(NATIVE "${CMAKE_CURRENT_SOURCE_DIR}/.." ABSOLUTE)

foreach(tool XZ XZDEC LZMADEC LZMAINFO SCRIPTS)
  set(XZ_TOOL_${tool} OFF CACHE BOOL "" FORCE)
endforeach()
set(XZ_NLS OFF CACHE BOOL "" FORCE)
set(XZ_DOC OFF CACHE BOOL "" FORCE)
add_subdirectory("${NATIVE}/xz" xz)
# Play! also installs an unrelated LZMA SDK archive named liblzma.a.
# Give libarchive's full XZ/LZMA library a distinct filename.
set_target_properties(liblzma PROPERTIES OUTPUT_NAME kog_liblzma)

set(ZSTD_BUILD_PROGRAMS OFF CACHE BOOL "" FORCE)
set(ZSTD_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(ZSTD_BUILD_SHARED OFF CACHE BOOL "" FORCE)
set(ZSTD_BUILD_STATIC ON CACHE BOOL "" FORCE)
set(ZSTD_MULTITHREAD_SUPPORT OFF CACHE BOOL "" FORCE)
add_subdirectory("${NATIVE}/play/deps/Dependencies/zstd/build/cmake" zstd)
set_target_properties(libzstd_static PROPERTIES OUTPUT_NAME kog_zstd)

add_library(bz2 STATIC "${NATIVE}/bzip2/blocksort.c" "${NATIVE}/bzip2/huffman.c"
  "${NATIVE}/bzip2/crctable.c" "${NATIVE}/bzip2/randtable.c" "${NATIVE}/bzip2/compress.c"
  "${NATIVE}/bzip2/decompress.c" "${NATIVE}/bzip2/bzlib.c")
add_library(lz4 STATIC "${NATIVE}/lz4/lib/lz4.c" "${NATIVE}/lz4/lib/lz4hc.c"
  "${NATIVE}/lz4/lib/lz4frame.c" "${NATIVE}/lz4/lib/xxhash.c")
# Avoid collisions with Play!'s separate xxHash library in the same executable.
target_compile_definitions(lz4 PRIVATE XXH_NAMESPACE=kog_lz4_)
install(TARGETS bz2 lz4 ARCHIVE DESTINATION lib)
install(FILES "${NATIVE}/bzip2/bzlib.h" "${NATIVE}/lz4/lib/lz4.h"
  "${NATIVE}/lz4/lib/lz4frame.h" "${NATIVE}/lz4/lib/lz4hc.h" DESTINATION include)
