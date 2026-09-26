cmake_minimum_required(VERSION 3.18)
project(kog_sfm_comparison LANGUAGES CXX)
# Compatibility build entry point. The historical GPL2-only wrapper below is
# no longer compiled with the rebased GPLv3 core; use the new renderer driver.
set(KOG_SFM_TEST_DRIVER ON CACHE BOOL "Build the compatible SFM comparison driver" FORCE)
add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/../sfm-embedded" "${CMAKE_CURRENT_BINARY_DIR}/embedded")
