cmake_minimum_required(VERSION 3.16)
project(kog_vgmstream LANGUAGES C)

# Build the pinned sources for the target, with no configure-time downloads or
# upstream Windows DLL imports. The same static codec set is used on mobile
# and desktop. FFmpeg headers/libraries come from kog-audio's existing probe.
set(CMAKE_POLICY_VERSION_MINIMUM 3.5)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(CMAKE_C_STANDARD 99)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(INSTALL_DOCS OFF CACHE BOOL "" FORCE)
get_filename_component(NATIVE "${CMAKE_CURRENT_SOURCE_DIR}/.." ABSOLUTE)
set(VGM "${NATIVE}/vgmstream")

add_subdirectory("${NATIVE}/ogg" ogg EXCLUDE_FROM_ALL)
set(OGG_INCLUDE_DIR "${NATIVE}/ogg/include" CACHE PATH "" FORCE)
set(OGG_LIBRARY ogg CACHE STRING "" FORCE)
add_subdirectory("${NATIVE}/vorbis" vorbis EXCLUDE_FROM_ALL)

# Portable floating-point MPEG avoids platform assembler/tool dependencies.
set(BUILD_LIBOUT123 OFF CACHE BOOL "" FORCE)
foreach(arch X86 X64 ARM32 ARM64)
  set(HAVE_ARCH_IS_${arch} FALSE)
  set(ARCH_IS_${arch} FALSE)
endforeach()
add_subdirectory("${NATIVE}/mpg123/ports/cmake" mpg123 EXCLUDE_FROM_ALL)

# Upstream's public header only declares DLL import/export on Windows. Patch
# an out-of-tree copy for static linkage; never modify the pinned checkout.
file(COPY "${NATIVE}/libatrac9/C/src/" DESTINATION "${CMAKE_CURRENT_BINARY_DIR}/atrac9")
file(READ "${NATIVE}/libatrac9/C/src/libatrac9.h" atrac9_header)
string(REPLACE "#ifdef _MSC_VER" "#if 0 /* Kog static library */" atrac9_header "${atrac9_header}")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/atrac9/libatrac9.h" "${atrac9_header}")
file(GLOB atrac9_sources "${CMAKE_CURRENT_BINARY_DIR}/atrac9/*.c")
add_library(kog_atrac9 STATIC ${atrac9_sources})
target_include_directories(kog_atrac9 PUBLIC "${CMAKE_CURRENT_BINARY_DIR}/atrac9")

set(speex_sources cb_search exc_10_32_table exc_8_128_table filters gain_table
  hexc_table high_lsp_tables lsp ltp speex stereo vbr vq bits exc_10_16_table
  exc_20_32_table exc_5_256_table exc_5_64_table gain_table_lbr hexc_10_32_table
  lpc lsp_tables_nb modes modes_wb nb_celp quant_lsp sb_celp speex_callbacks
  speex_header window)
list(TRANSFORM speex_sources PREPEND "${NATIVE}/speex/libspeex/")
list(TRANSFORM speex_sources APPEND ".c")
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/speex_config_types.h"
  "#include <stdint.h>\ntypedef int16_t spx_int16_t;\ntypedef uint16_t spx_uint16_t;\ntypedef int32_t spx_int32_t;\ntypedef uint32_t spx_uint32_t;\n")
add_library(kog_speex STATIC ${speex_sources})
target_include_directories(kog_speex PUBLIC "${NATIVE}/speex/include" PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
target_compile_definitions(kog_speex PRIVATE FLOATING_POINT USE_ALLOCA EXPORT=
  SPEEX_MAJOR_VERSION=1 SPEEX_MINOR_VERSION=2 SPEEX_MICRO_VERSION=1
  SPEEX_EXTRA_VERSION="" SPEEX_VERSION="1.2.1")

include(celt.cmake)

# Keep upstream's source layout, including every game-container parser.
file(GLOB vgm_sources "${VGM}/src/*.c" "${VGM}/src/base/*.c"
  "${VGM}/src/coding/*.c" "${VGM}/src/coding/libs/*.c" "${VGM}/src/layout/*.c"
  "${VGM}/src/meta/*.c" "${VGM}/src/util/*.c")
add_library(kog_vgmstream STATIC ${vgm_sources})
target_compile_definitions(kog_vgmstream PRIVATE VGM_USE_MPEG VGM_USE_VORBIS
  VGM_USE_FFMPEG VGM_USE_G7221 VGM_USE_ATRAC9 VGM_USE_CELT VGM_USE_SPEEX
  MPG123_STATIC)
target_include_directories(kog_vgmstream PRIVATE ${KOG_FFMPEG_INCLUDE_DIRS}
  "${VGM}/src" "${CMAKE_CURRENT_BINARY_DIR}/atrac9")
target_link_libraries(kog_vgmstream PRIVATE libmpg123 vorbisfile vorbis ogg
  kog_atrac9 kog_speex kog_celt0061 kog_celt0110)
# Last: bundled compatibility headers must not shadow the real codec headers.
target_include_directories(kog_vgmstream PRIVATE "${VGM}/ext_includes")
if(WIN32)
  target_compile_definitions(kog_vgmstream PRIVATE WIN32 _CRT_SECURE_NO_WARNINGS VGM_STDIO_UNICODE)
endif()
if(NOT MSVC)
  target_compile_options(kog_vgmstream PRIVATE -Wno-deprecated-declarations)
endif()
install(TARGETS kog_vgmstream libmpg123 vorbisfile vorbis ogg kog_atrac9
  kog_speex kog_celt0061 kog_celt0110 ARCHIVE DESTINATION lib)

option(KOG_CODEC_TESTS "Run native codec regression fixtures" OFF)
if(KOG_CODEC_TESTS)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(FFMPEG REQUIRED IMPORTED_TARGET libavformat libavcodec libavutil libswresample)
  add_executable(kog-codec-probe "${NATIVE}/../tests/native/codec_probe.c" "${NATIVE}/vgmstream_bridge.c")
  foreach(version 0061 0110)
    add_library(celt_probe_${version} OBJECT "${NATIVE}/../tests/native/celt_probe.c")
    get_target_property(celt_definitions kog_celt${version} COMPILE_DEFINITIONS)
    target_compile_definitions(celt_probe_${version} PRIVATE ${celt_definitions}
      KOG_CELT_PROBE=kog_celt_probe_${version} KOG_CELT_OLD=$<STREQUAL:${version},0061>)
    target_include_directories(celt_probe_${version} PRIVATE "${NATIVE}/celt-${version}/libcelt")
    target_sources(kog-codec-probe PRIVATE $<TARGET_OBJECTS:celt_probe_${version}>)
  endforeach()
  target_include_directories(kog-codec-probe PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/atrac9"
    "${NATIVE}" "${VGM}/src" "${VGM}/ext_includes")
  target_link_libraries(kog-codec-probe PRIVATE kog_vgmstream kog_speex kog_atrac9 PkgConfig::FFMPEG)
  if(NOT WIN32)
    target_link_libraries(kog-codec-probe PRIVATE m)
  endif()
  enable_testing()
  add_test(NAME codec-playback COMMAND kog-codec-probe "${NATIVE}/../tests/fixtures/codec-libs")
endif()
