cmake_minimum_required(VERSION 3.18)
project(kog_psf2_helper LANGUAGES C CXX)

if(NOT DEFINED PLAY_SOURCE OR NOT EXISTS "${PLAY_SOURCE}/License.txt")
    message(FATAL_ERROR "PLAY_SOURCE must point at the pinned Play! checkout")
endif()

set(CMAKE_MODULE_PATH
    "${PLAY_SOURCE}/deps/Dependencies/cmake-modules"
    ${CMAKE_MODULE_PATH}
)
include(Header)

set(ENABLE_AMAZON_S3 OFF CACHE BOOL "Disable unrelated Play! network support" FORCE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build static dependencies" FORCE)
# libchdr's bundled zlib configuration renames a tracked source header in an
# out-of-tree build. The platform zlib is already a required PSF2 dependency.
set(WITH_SYSTEM_ZLIB ON CACHE BOOL "Keep the pinned Play! source immutable" FORCE)
add_subdirectory("${PLAY_SOURCE}/Source" "${CMAKE_CURRENT_BINARY_DIR}/PlayCore")

# vcpkg's dynamic zlib marks consumers with ZLIB_DLL, which turns zlib.h's
# ZEXTERN declarations into dllimport. zstd's wrapper defines those z_* entry
# points itself, so it must compile as the zlib-internal side of the DLL.
if(TARGET libzstd_zlibwrapper_static)
    target_compile_definitions(libzstd_zlibwrapper_static PRIVATE ZLIB_INTERNAL)
endif()

# Play! 04bde0d uses memcpy in BasicBlock.cpp without directly including the
# standard declaration. Force-include the header until that pinned upstream
# revision is advanced. This changes no emulator behavior.
if(MSVC)
    target_compile_options(PlayCore PRIVATE /FIcstring)
else()
    target_compile_options(PlayCore PRIVATE -include cstring)
endif()

set(PSFPLAYER_SOURCE "${PLAY_SOURCE}/tools/PsfPlayer/Source")
add_library(kog_psf2_play_core STATIC
    "${PSFPLAYER_SOURCE}/Iop_PsfSubSystem.cpp"
    "${PSFPLAYER_SOURCE}/PsfBase.cpp"
    "${PSFPLAYER_SOURCE}/PsfFs.cpp"
    "${PSFPLAYER_SOURCE}/ps2/Ps2_PsfDevice.cpp"
)
target_compile_features(kog_psf2_play_core PRIVATE cxx_std_17)
target_include_directories(kog_psf2_play_core SYSTEM PUBLIC
    "${PSFPLAYER_SOURCE}"
    "${PLAY_SOURCE}/Source"
)
target_link_libraries(kog_psf2_play_core PUBLIC PlayCore)
if(MSVC)
    # MSVC only implements C++ exception unwinding with /EHsc.
    target_compile_options(kog_psf2_play_core PRIVATE /EHsc)
endif()

add_executable(kog-psf2-helper psf2_helper.cpp)
target_compile_features(kog-psf2-helper PRIVATE cxx_std_17)
target_include_directories(kog-psf2-helper SYSTEM PRIVATE
    "${PSFPLAYER_SOURCE}"
    "${PLAY_SOURCE}/Source"
    "${PLAY_SOURCE}/deps/Dependencies/zstd/zlibWrapper"
)
target_link_libraries(kog-psf2-helper PRIVATE kog_psf2_play_core)
if(MSVC)
    target_compile_options(kog-psf2-helper PRIVATE /EHsc /W4 /WX)
else()
    target_compile_options(kog-psf2-helper PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

install(TARGETS kog-psf2-helper RUNTIME DESTINATION bin)
