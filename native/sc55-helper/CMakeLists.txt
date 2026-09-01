cmake_minimum_required(VERSION 3.19)
project(kog_sc55_helper LANGUAGES C CXX)

if(NOT DEFINED NUKED_SC55_SOURCE OR
   NOT EXISTS "${NUKED_SC55_SOURCE}/src/backend/emu.h")
    message(FATAL_ERROR
        "NUKED_SC55_SOURCE must point at the pinned Nuked SC-55 checkout")
endif()

set(SC55_BACKEND "${NUKED_SC55_SOURCE}/src/backend")
set(SC55_COMMON "${NUKED_SC55_SOURCE}/src/common")

add_library(kog_sc55_core STATIC
    "${SC55_BACKEND}/emu.cpp"
    "${SC55_BACKEND}/lcd.cpp"
    "${SC55_BACKEND}/mcu.cpp"
    "${SC55_BACKEND}/mcu_interrupt.cpp"
    "${SC55_BACKEND}/mcu_opcodes.cpp"
    "${SC55_BACKEND}/mcu_timer.cpp"
    "${SC55_BACKEND}/pcm.cpp"
    "${SC55_BACKEND}/rom.cpp"
    "${SC55_BACKEND}/rom_io.cpp"
    "${SC55_BACKEND}/submcu.cpp"
    "${SC55_BACKEND}/sha/sha224-256.c"
    "${SC55_COMMON}/rom_loader.cpp"
)
target_compile_features(kog_sc55_core PRIVATE cxx_std_23)
target_include_directories(kog_sc55_core SYSTEM PUBLIC
    "${SC55_BACKEND}"
    "${SC55_COMMON}"
)

add_executable(kog-sc55-helper sc55_helper.cpp)
target_compile_features(kog-sc55-helper PRIVATE cxx_std_23)
target_link_libraries(kog-sc55-helper PRIVATE kog_sc55_core)
if(MSVC)
    target_compile_options(kog-sc55-helper PRIVATE /W4 /WX)
else()
    target_compile_options(kog-sc55-helper PRIVATE
        -Wall -Wextra -Wpedantic -Werror)
endif()

install(TARGETS kog-sc55-helper RUNTIME DESTINATION bin)
