cmake_minimum_required(VERSION 3.19)
project(kog_sc55_helper VERSION 0.7.0 LANGUAGES C CXX)

if(NOT DEFINED NUKED_SC55_SOURCE OR
   NOT EXISTS "${NUKED_SC55_SOURCE}/src/backend/emu.h")
    message(FATAL_ERROR
        "NUKED_SC55_SOURCE must point at the pinned Nuked SC-55 checkout")
endif()

set(SC55_BACKEND "${NUKED_SC55_SOURCE}/src/backend")
set(SC55_COMMON "${NUKED_SC55_SOURCE}/src/common")
set(NUKED_ENABLE_DECODER2 OFF)
set(NUKED_ENABLE_ASIO OFF)
set(NUKED_SOURCE "Kog")
configure_file("${SC55_BACKEND}/config.h.in" "${CMAKE_CURRENT_BINARY_DIR}/config.h" @ONLY)

add_library(kog_sc55_core STATIC
    "${SC55_BACKEND}/config.cpp"
    "${SC55_BACKEND}/diagnostics.cpp"
    "${SC55_BACKEND}/emu.cpp"
    "${SC55_BACKEND}/file_hashing.cpp"
    "${SC55_BACKEND}/file_io.cpp"
    "${SC55_BACKEND}/lcd.cpp"
    "${SC55_BACKEND}/mcu.cpp"
    "${SC55_BACKEND}/mcu_interrupt.cpp"
    "${SC55_BACKEND}/mcu_opcodes.cpp"
    "${SC55_BACKEND}/mcu_timer.cpp"
    "${SC55_BACKEND}/pcm.cpp"
    "${SC55_BACKEND}/rom.cpp"
    "${SC55_BACKEND}/rom_io.cpp"
    "${SC55_BACKEND}/standard_romsets.cpp"
    "${SC55_BACKEND}/submcu.cpp"
    "${SC55_BACKEND}/sha/sha224-256.c"
    "${SC55_COMMON}/rom_loader.cpp"
    "${SC55_COMMON}/path_util.cpp"
    "${SC55_COMMON}/term_io.cpp"
)
target_compile_features(kog_sc55_core PRIVATE cxx_std_23)
if(MSVC)
    # MSVC only implements C++ exception unwinding with /EHsc.
    target_compile_options(kog_sc55_core PRIVATE /EHsc)
endif()
target_include_directories(kog_sc55_core SYSTEM PUBLIC
    "${SC55_BACKEND}"
    "${SC55_COMMON}"
    "${NUKED_SC55_SOURCE}/src"
    "${CMAKE_CURRENT_BINARY_DIR}"
)

option(KOG_SC55_MOBILE "Build the in-process mobile renderer" OFF)
if(KOG_SC55_MOBILE)
    add_library(kog_sc55_embedded STATIC sc55_helper.cpp)
    target_compile_definitions(kog_sc55_embedded PRIVATE KOG_SC55_EMBEDDED=1)
    set(SC55_TARGET kog_sc55_embedded)
else()
    add_executable(kog-sc55-helper sc55_helper.cpp)
    set(SC55_TARGET kog-sc55-helper)
endif()
target_compile_features(${SC55_TARGET} PRIVATE cxx_std_23)
target_link_libraries(${SC55_TARGET} PRIVATE kog_sc55_core)
if(WIN32)
    target_link_libraries(${SC55_TARGET} PRIVATE ws2_32)
endif()
if(MSVC)
    # windows.h's min/max macros collide with std::max/std::min below.
    target_compile_definitions(${SC55_TARGET} PRIVATE NOMINMAX)
    target_compile_options(${SC55_TARGET} PRIVATE /EHsc /W4 /WX)
else()
    target_compile_options(${SC55_TARGET} PRIVATE
        -Wall -Wextra -Wpedantic -Werror)
endif()

if(KOG_SC55_MOBILE)
    install(TARGETS kog_sc55_core kog_sc55_embedded ARCHIVE DESTINATION lib)
else()
    install(TARGETS kog-sc55-helper RUNTIME DESTINATION bin)
endif()
