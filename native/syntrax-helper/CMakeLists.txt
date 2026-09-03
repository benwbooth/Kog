cmake_minimum_required(VERSION 3.18)
project(kog_syntrax_helper LANGUAGES C CXX)

if(NOT DEFINED SYNTRAX_SOURCE OR NOT EXISTS "${SYNTRAX_SOURCE}/jaytrax.c")
    message(FATAL_ERROR "SYNTRAX_SOURCE must point at the pinned syntrax-c checkout")
endif()

# Upstream's public jxs.h retains its original Apple framework include path.
# Supply that layout in the build tree while leaving the pinned checkout intact.
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/include/Syntrax_c")
configure_file("${SYNTRAX_SOURCE}/jaytrax.h"
               "${CMAKE_CURRENT_BINARY_DIR}/include/Syntrax_c/jaytrax.h" COPYONLY)

add_library(kog_syntrax_core STATIC
    "${SYNTRAX_SOURCE}/jaytrax.c"
    "${SYNTRAX_SOURCE}/jxs.c"
    "${SYNTRAX_SOURCE}/mixcore.c"
)
target_compile_features(kog_syntrax_core PRIVATE c_std_11)
target_include_directories(kog_syntrax_core SYSTEM PUBLIC
    "${SYNTRAX_SOURCE}"
    "${CMAKE_CURRENT_BINARY_DIR}/include"
)
if(NOT MSVC)
    target_compile_options(kog_syntrax_core PRIVATE -fwrapv)
endif()

add_executable(kog-syntrax-helper syntrax_helper.cpp)
target_compile_features(kog-syntrax-helper PRIVATE cxx_std_17)
target_link_libraries(kog-syntrax-helper PRIVATE kog_syntrax_core)
if(MSVC)
    # windows.h's min/max macros collide with std::max/std::min below.
    target_compile_definitions(kog-syntrax-helper PRIVATE NOMINMAX)
    target_compile_options(kog-syntrax-helper PRIVATE /EHsc /W4 /WX)
else()
    target_compile_options(kog-syntrax-helper PRIVATE -Wall -Wextra -Wpedantic -Werror)
    target_link_libraries(kog-syntrax-helper PRIVATE m)
endif()

install(TARGETS kog-syntrax-helper RUNTIME DESTINATION bin)
