cmake_minimum_required(VERSION 3.18)
project(kog_snsf_helper LANGUAGES C CXX)

if(NOT DEFINED LIBSNSF9X_SOURCE OR NOT EXISTS "${LIBSNSF9X_SOURCE}/snsf9x.h")
    message(FATAL_ERROR "LIBSNSF9X_SOURCE must point at the pinned libsnsf9x checkout")
endif()
if(NOT DEFINED PSFLIB_SOURCE OR NOT EXISTS "${PSFLIB_SOURCE}/psflib.h")
    message(FATAL_ERROR "PSFLIB_SOURCE must point at the pinned psflib checkout")
endif()

find_package(ZLIB REQUIRED)

# GCC 15 diagnoses the old framework's `const LPVOID` declaration as a hard
# function-pointer type mismatch (`void *const` versus `const void *`). Keep the
# pin immutable and generate the one-token const-correct compatibility copy.
file(READ "${LIBSNSF9X_SOURCE}/xsfc/xsfdrv.c" XSFDRV_SOURCE_TEXT)
string(REPLACE "const LPVOID lpPtr" "const void *lpPtr"
       XSFDRV_SOURCE_TEXT "${XSFDRV_SOURCE_TEXT}")
# The upstream file is normally a shared-library entry point. Kog links it
# statically into an executable, where DllMain is neither used nor valid
# without the original Windows plugin framework headers.
set(XSFDRV_DLLMAIN [=[#ifdef _WIN32
BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved)
{
	if (fdwReason == DLL_PROCESS_ATTACH)
	{
#if defined(_MSC_VER) && defined(_DEBUG)
		_CrtSetDbgFlag(_CRTDBG_ALLOC_MEM_DF | _CRTDBG_CHECK_ALWAYS_DF | _CRTDBG_LEAK_CHECK_DF);
#endif
		DisableThreadLibraryCalls(hinstDLL);
	}
	return TRUE;
}
#endif]=])
string(REPLACE "${XSFDRV_DLLMAIN}" "" XSFDRV_SOURCE_TEXT "${XSFDRV_SOURCE_TEXT}")
set(XSFDRV_SOURCE "${CMAKE_CURRENT_BINARY_DIR}/xsfdrv.c")
file(WRITE "${XSFDRV_SOURCE}" "${XSFDRV_SOURCE_TEXT}")

# Build the upstream library source list exactly as its GNUmakefile does. The
# library has no CMake project of its own, apart from the const-correct copy
# above required by current compilers.
add_library(kog_libsnsf9x STATIC
    "${LIBSNSF9X_SOURCE}/snsf9x/drvimpl.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/apu/apu.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/apu/SNES_SPC.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/apu/SNES_SPC_misc.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/apu/SPC_DSP.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/cpu.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/cpuexec.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/cpuops.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/dma.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/globals.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/memmap.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/ppu.cpp"
    "${LIBSNSF9X_SOURCE}/snsf9x/snes9x/sdd1.cpp"
    "${XSFDRV_SOURCE}"
)
target_compile_features(kog_libsnsf9x PRIVATE c_std_11)
set_target_properties(kog_libsnsf9x PROPERTIES CXX_STANDARD 11 CXX_STANDARD_REQUIRED YES)
target_include_directories(kog_libsnsf9x SYSTEM PUBLIC
    "${LIBSNSF9X_SOURCE}"
    "${LIBSNSF9X_SOURCE}/in_xsf_framework"
    "${LIBSNSF9X_SOURCE}/xsfc"
)
target_link_libraries(kog_libsnsf9x PUBLIC ZLIB::ZLIB)
if(NOT MSVC)
    target_compile_options(kog_libsnsf9x PRIVATE -fwrapv)
else()
    # MSVC only implements C++ exception unwinding with /EHsc. The framework
    # also expects a Unicode windows.h (its wrapper rejects the MBCS default
    # that CMake's VS projects use) and spells strncasecmp as _strnicmp.
    target_compile_options(kog_libsnsf9x PRIVATE /EHsc)
    target_compile_definitions(kog_libsnsf9x PRIVATE
        UNICODE _UNICODE strncasecmp=_strnicmp strcasecmp=_stricmp)
endif()

add_library(kog_snsf_psflib STATIC "${PSFLIB_SOURCE}/psflib.c")
target_compile_features(kog_snsf_psflib PRIVATE c_std_11)
target_include_directories(kog_snsf_psflib SYSTEM PUBLIC "${PSFLIB_SOURCE}")
target_link_libraries(kog_snsf_psflib PUBLIC ZLIB::ZLIB)

add_executable(kog-snsf-helper snsf_helper.cpp)
target_compile_features(kog-snsf-helper PRIVATE cxx_std_17)
target_include_directories(kog-snsf-helper SYSTEM PRIVATE
    "${LIBSNSF9X_SOURCE}"
    "${LIBSNSF9X_SOURCE}/in_xsf_framework"
    "${PSFLIB_SOURCE}"
)
target_link_libraries(kog-snsf-helper PRIVATE kog_libsnsf9x kog_snsf_psflib ZLIB::ZLIB)
if(MSVC)
    # The helper calls _wfopen directly, and windows.h's min/max macros collide
    # with std::max/std::min below; MSVC's notices about the former are not
    # worth failing /WX over.
    target_compile_definitions(kog-snsf-helper PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(kog-snsf-helper PRIVATE /EHsc /W4 /WX)
else()
    target_compile_options(kog-snsf-helper PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

install(TARGETS kog-snsf-helper RUNTIME DESTINATION bin)
