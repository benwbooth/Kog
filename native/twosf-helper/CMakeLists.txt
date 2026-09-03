cmake_minimum_required(VERSION 3.18)
project(kog_twosf_helper LANGUAGES C CXX)

if(NOT DEFINED MELONDS_SOURCE OR NOT EXISTS "${MELONDS_SOURCE}/src/NDS.h")
    message(FATAL_ERROR "MELONDS_SOURCE must point at the pinned melonDS checkout")
endif()
if(NOT DEFINED PSFLIB_SOURCE OR NOT EXISTS "${PSFLIB_SOURCE}/psflib.h")
    message(FATAL_ERROR "PSFLIB_SOURCE must point at the pinned psflib checkout")
endif()

# Kog needs the maintained emulation core, not melonDS's Qt/SDL application.
set(BUILD_QT_SDL OFF CACHE BOOL "Do not build the melonDS frontend" FORCE)
set(ENABLE_GDBSTUB OFF CACHE BOOL "Do not build debugger support" FORCE)
set(ENABLE_JIT OFF CACHE BOOL "Use the deterministic portable interpreter" FORCE)
set(ENABLE_OGLRENDERER OFF CACHE BOOL "Do not build video OpenGL support" FORCE)
set(ENABLE_LTO_RELEASE OFF CACHE BOOL "Keep Cargo builds predictable" FORCE)
set(ENABLE_LTO OFF CACHE BOOL "Keep Cargo builds predictable" FORCE)
set(USE_VCPKG OFF CACHE BOOL "Use system build dependencies" FORCE)
# melonDS's non-MinGW Windows path requires a dirent.h on its include path,
# which MSVC does not ship. Preempt its find_path() with Kog's vendored shim.
if(MSVC)
    set(DIRENT_INCLUDE_DIRS "${CMAKE_CURRENT_SOURCE_DIR}/dirent"
        CACHE PATH "dirent.h shim for melonDS" FORCE)
endif()
add_subdirectory("${MELONDS_SOURCE}" "${CMAKE_CURRENT_BINARY_DIR}/melonDS")

find_package(ZLIB REQUIRED)

add_library(kog_twosf_psflib STATIC "${PSFLIB_SOURCE}/psflib.c")
target_compile_features(kog_twosf_psflib PRIVATE c_std_11)
target_include_directories(kog_twosf_psflib SYSTEM PUBLIC "${PSFLIB_SOURCE}")
target_link_libraries(kog_twosf_psflib PUBLIC ZLIB::ZLIB)

add_executable(kog-2sf-helper twosf_helper.cpp platform.cpp)
target_compile_features(kog-2sf-helper PRIVATE cxx_std_17)
target_include_directories(kog-2sf-helper SYSTEM PRIVATE
    "${MELONDS_SOURCE}/src"
    "${PSFLIB_SOURCE}"
)
target_link_libraries(kog-2sf-helper PRIVATE core kog_twosf_psflib ZLIB::ZLIB)
if(CMAKE_DL_LIBS)
    target_link_libraries(kog-2sf-helper PRIVATE ${CMAKE_DL_LIBS})
endif()
if(MSVC)
    # The helper calls _wfopen directly, and windows.h's min/max macros collide
    # with std::max/std::min below; MSVC's notices about the former are not
    # worth failing /WX over.
    target_compile_definitions(kog-2sf-helper PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(kog-2sf-helper PRIVATE /EHsc /W4 /WX)
else()
    target_compile_options(kog-2sf-helper PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

install(TARGETS kog-2sf-helper RUNTIME DESTINATION bin)
