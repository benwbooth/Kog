# Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
# Injected into the pinned, untouched Furnace checkout's own configure step
# (CMAKE_PROJECT_furnace_INCLUDE). Furnace assumes it is the top-level
# project, so it is configured headless as itself; once its `furnace` target
# exists, Kog's static library reuses that target's engine sources, includes,
# definitions and links, minus the CLI front end (no audio device backend
# is enabled in this headless configuration).
set(KOG_FURNACE_BRIDGE_DIR "${CMAKE_CURRENT_LIST_DIR}")

function(kog_furnace_library)
    get_target_property(furnace_sources furnace SOURCES)
    set(engine_sources)
    foreach(source IN LISTS furnace_sources)
        if(source MATCHES "src/main\\.cpp$|src/cli/|\\.rc$|\\.xml$")
            continue()
        endif()
        if(NOT IS_ABSOLUTE "${source}")
            set(source "${CMAKE_SOURCE_DIR}/${source}")
        endif()
        list(APPEND engine_sources "${source}")
    endforeach()

    add_library(kog_furnace STATIC ${engine_sources}
        "${KOG_FURNACE_BRIDGE_DIR}/furnace_bridge.cpp")
    foreach(property INCLUDE_DIRECTORIES COMPILE_DEFINITIONS COMPILE_OPTIONS)
        get_target_property(value furnace ${property})
        if(value)
            set_property(TARGET kog_furnace PROPERTY ${property} "${value}")
        endif()
    endforeach()
    get_target_property(furnace_links furnace LINK_LIBRARIES)
    if(furnace_links)
        target_link_libraries(kog_furnace PUBLIC ${furnace_links})
    endif()

    # The bridge drives the engine's internals the way its export thread
    # does. A copy of engine.h that only befriends the bridge (no layout
    # change) is searched first; everything else is the untouched checkout.
    file(READ "${CMAKE_SOURCE_DIR}/src/engine/engine.h" engine_header)
    string(FIND "${engine_header}" "class DivEngine {" engine_class)
    if(engine_class LESS 0)
        message(FATAL_ERROR "Pinned Furnace engine.h no longer matches the Kog bridge")
    endif()
    string(REPLACE "class DivEngine {" "class DivEngine {\n  friend struct KogFurnaceAccess;" engine_header "${engine_header}")
    file(WRITE "${CMAKE_BINARY_DIR}/kog-include/engine/engine.h" "${engine_header}")
    target_include_directories(kog_furnace BEFORE PRIVATE "${CMAKE_BINARY_DIR}/kog-include")
    target_include_directories(kog_furnace PRIVATE "${CMAKE_SOURCE_DIR}/src" "${CMAKE_SOURCE_DIR}/src/engine")
    # Furnace's sources rely on its precompiled header for standard includes.
    target_precompile_headers(kog_furnace PRIVATE "$<$<COMPILE_LANGUAGE:CXX>:${CMAKE_SOURCE_DIR}/src/pch.h>")
    set_target_properties(kog_furnace PROPERTIES POSITION_INDEPENDENT_CODE ON)
    install(TARGETS kog_furnace ARCHIVE DESTINATION lib)
endfunction()

cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL kog_furnace_library)
