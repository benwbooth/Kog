cmake_minimum_required(VERSION 3.18)

project(kog_psf_helper LANGUAGES C)

if(NOT DEFINED UPSE_SOURCE OR NOT EXISTS "${UPSE_SOURCE}/upse.h")
    message(FATAL_ERROR "UPSE_SOURCE must point at the pinned libupse checkout")
endif()

find_package(ZLIB REQUIRED)
include(TestBigEndian)
test_big_endian(KOG_BIG_ENDIAN)

set(UPSE_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/upse_compat.c"
    "${UPSE_SOURCE}/Neill/spu.c"
    "${UPSE_SOURCE}/Neill/spucore.c"
    "${UPSE_SOURCE}/upse_bios_manager.c"
    "${UPSE_SOURCE}/upse_container_xsf.c"
    "${UPSE_SOURCE}/upse_eventloop.c"
    "${UPSE_SOURCE}/upse_filesystem.c"
    "${UPSE_SOURCE}/upse_loader.c"
    "${UPSE_SOURCE}/upse_loader_psf1.c"
    "${UPSE_SOURCE}/upse_loader_psf2.c"
    "${UPSE_SOURCE}/upse_module.c"
    "${UPSE_SOURCE}/upse_ps1_bios_base.c"
    "${UPSE_SOURCE}/upse_ps1_counters.c"
    "${UPSE_SOURCE}/upse_ps1_dma_manager.c"
    "${UPSE_SOURCE}/upse_ps1_executive.c"
    "${UPSE_SOURCE}/upse_ps1_gpu.c"
    "${UPSE_SOURCE}/upse_ps1_hal.c"
    "${UPSE_SOURCE}/upse_ps1_memory_manager.c"
    "${UPSE_SOURCE}/upse_ps1_spu_base.c"
    "${UPSE_SOURCE}/upse_ps1_spu_dma_handler.c"
    "${UPSE_SOURCE}/upse_ps1_spu_lowpass_filter.c"
    "${UPSE_SOURCE}/upse_ps1_spu_nyquist_filter.c"
    "${UPSE_SOURCE}/upse_ps1_spu_register_io.c"
    "${UPSE_SOURCE}/upse_ps2_iop_base.c"
    "${UPSE_SOURCE}/upse_r3000_abstract.c"
    "${UPSE_SOURCE}/upse_r3000_bytecode_evaluator.c"
    "${UPSE_SOURCE}/upse_string.c"
    "${UPSE_SOURCE}/upse_util.c"
)

add_library(kog_libupse STATIC ${UPSE_SOURCES})
target_include_directories(kog_libupse PUBLIC "${UPSE_SOURCE}")
target_compile_features(kog_libupse PRIVATE c_std_11)
set_target_properties(kog_libupse PROPERTIES C_EXTENSIONS ON)
target_compile_definitions(kog_libupse PRIVATE EMU_COMPILE)

if(KOG_BIG_ENDIAN)
    target_compile_definitions(kog_libupse PRIVATE EMU_BIG_ENDIAN)
else()
    target_compile_definitions(kog_libupse PRIVATE EMU_LITTLE_ENDIAN LSB_FIRST=1)
endif()

if(WIN32)
    target_compile_definitions(kog_libupse PRIVATE PSS_STYLE=3)
    if(MSVC)
        target_compile_definitions(kog_libupse PRIVATE WIN32_MSC WIN32)
        target_compile_options(kog_libupse PRIVATE /w)
    endif()
else()
    target_compile_definitions(kog_libupse PRIVATE PSS_STYLE=1)
endif()

if(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(kog_libupse PRIVATE -w)
endif()

add_executable(kog-psf-helper psf_helper.c)
target_compile_features(kog-psf-helper PRIVATE c_std_11)
target_link_libraries(kog-psf-helper PRIVATE kog_libupse ZLIB::ZLIB)
if(NOT WIN32)
    target_link_libraries(kog-psf-helper PRIVATE m)
endif()

if(MSVC)
    target_compile_options(kog-psf-helper PRIVATE /W4 /WX)
elseif(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(kog-psf-helper PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

install(TARGETS kog-psf-helper RUNTIME DESTINATION bin)
