cmake_minimum_required(VERSION 3.18)
project(kog_sfm_embedded LANGUAGES CXX)
include(TestBigEndian)
test_big_endian(KOG_SFM_BIG_ENDIAN)
if(NOT DEFINED COG_GME_SFM_SOURCE)
    get_filename_component(COG_GME_SFM_SOURCE "../cog-gme-sfm" ABSOLUTE BASE_DIR "${CMAKE_CURRENT_SOURCE_DIR}")
endif()
set(GME "${COG_GME_SFM_SOURCE}/gme")
set(HIGAN "${COG_GME_SFM_SOURCE}/higan")
add_library(kog_sfm_core STATIC
    "${GME}/Blip_Buffer.cpp" "${GME}/Bml_Parser.cpp" "${GME}/Data_Reader.cpp"
    "${GME}/Fir_Resampler.cpp" "${GME}/Gme_File.cpp" "${GME}/M3u_Playlist.cpp"
    "${GME}/Multi_Buffer.cpp" "${GME}/Music_Emu.cpp" "${GME}/Spc_Filter.cpp" "${GME}/Spc_Sfm.cpp"
    "${HIGAN}/processor/spc700/spc700.cpp" "${HIGAN}/smp/smp.cpp"
    "${HIGAN}/dsp/dsp.cpp" "${HIGAN}/dsp/SPC_DSP.cpp")
target_compile_features(kog_sfm_core PUBLIC cxx_std_17)
set_target_properties(kog_sfm_core PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_include_directories(kog_sfm_core SYSTEM PUBLIC "${COG_GME_SFM_SOURCE}" "${GME}")
# Cog's older GME ABI is different from Kog's libgme. Keep every public class,
# C function and template private to this decoder, including inline definitions.
foreach(symbol
    Blip_Buffer Blip_Synth_Fast_ Blip_Synth_ Blip_Synth Blip_Reader blip_eq_t Silent_Blip_Buffer
    blargg_vector Data_Reader File_Reader Std_File_Reader Mem_File_Reader Subset_Reader
    Remaining_Reader Callback_Reader Bml_Node Bml_Parser Fir_Resampler_ Fir_Resampler
    Gme_File Gme_Info_ Music_Emu Multi_Buffer Mono_Buffer Stereo_Buffer Silent_Buffer
    M3u_Playlist SPC_Filter Sfm_Emu Sfm_File track_info_t gme_info_t gme_type_t_
    gme_equalizer_t gme_type_t gme_wrong_file_type gme_sfm_type
    gme_load_m3u gme_load_m3u_data strchr_limited)
    target_compile_definitions(kog_sfm_core PUBLIC "${symbol}=KogSfm_${symbol}")
endforeach()
if(KOG_SFM_BIG_ENDIAN)
    target_compile_definitions(kog_sfm_core PUBLIC BLARGG_BIG_ENDIAN=1)
else()
    target_compile_definitions(kog_sfm_core PUBLIC BLARGG_LITTLE_ENDIAN=1)
endif()
if(MSVC)
    target_compile_options(kog_sfm_core PRIVATE /EHsc)
else()
    target_compile_options(kog_sfm_core PRIVATE -fwrapv -fno-strict-aliasing -w)
endif()
add_library(kog_sfm_embedded STATIC sfm_embedded.cpp)
target_compile_definitions(kog_sfm_embedded PRIVATE KOG_EMBEDDED=1)
set_target_properties(kog_sfm_embedded PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_link_libraries(kog_sfm_embedded PRIVATE kog_sfm_core)
if(MSVC)
    target_compile_definitions(kog_sfm_embedded PRIVATE NOMINMAX _CRT_SECURE_NO_WARNINGS)
    target_compile_options(kog_sfm_embedded PRIVATE /EHsc)
endif()
install(TARGETS kog_sfm_embedded kog_sfm_core ARCHIVE DESTINATION lib)
option(KOG_SFM_TEST_DRIVER "Build standalone comparison driver" OFF)
if(KOG_SFM_TEST_DRIVER)
    add_executable(kog-sfm-render sfm_embedded.cpp)
    target_compile_definitions(kog-sfm-render PRIVATE KOG_SFM_TEST_DRIVER=1)
    target_link_libraries(kog-sfm-render PRIVATE kog_sfm_core)
endif()
option(KOG_SFM_BUILD_TESTS "Build bounded SFM core tests" OFF)
if(KOG_SFM_BUILD_TESTS)
    enable_testing()
    add_executable(kog-sfm-core-tests core_tests.cpp)
    target_link_libraries(kog-sfm-core-tests PRIVATE kog_sfm_core)
    add_test(NAME sfm-core COMMAND kog-sfm-core-tests)
    set_tests_properties(sfm-core PROPERTIES TIMEOUT 10)
endif()
