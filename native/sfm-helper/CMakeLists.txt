cmake_minimum_required(VERSION 3.18)
project(kog_sfm_helper LANGUAGES CXX)

include(TestBigEndian)
test_big_endian(KOG_SFM_BIG_ENDIAN)

if(NOT DEFINED COG_GME_SFM_SOURCE OR
   NOT EXISTS "${COG_GME_SFM_SOURCE}/gme/Spc_Sfm.cpp")
    message(FATAL_ERROR "COG_GME_SFM_SOURCE must point at Kog's pinned Cog GME SFM subset")
endif()

set(GME "${COG_GME_SFM_SOURCE}/gme")
set(HIGAN "${COG_GME_SFM_SOURCE}/higan")

add_library(kog_sfm_core STATIC
    "${GME}/Blip_Buffer.cpp"
    "${GME}/Bml_Parser.cpp"
    "${GME}/Data_Reader.cpp"
    "${GME}/Fir_Resampler.cpp"
    "${GME}/Gme_File.cpp"
    "${GME}/M3u_Playlist.cpp"
    "${GME}/Multi_Buffer.cpp"
    "${GME}/Music_Emu.cpp"
    "${GME}/Spc_Filter.cpp"
    "${GME}/Spc_Sfm.cpp"
    "${HIGAN}/processor/spc700/spc700.cpp"
    "${HIGAN}/smp/smp.cpp"
    "${HIGAN}/dsp/dsp.cpp"
    "${HIGAN}/dsp/SPC_DSP.cpp"
)
target_compile_features(kog_sfm_core PRIVATE cxx_std_17)
target_include_directories(kog_sfm_core SYSTEM PUBLIC
    "${COG_GME_SFM_SOURCE}"
    "${GME}"
)
if(KOG_SFM_BIG_ENDIAN)
    target_compile_definitions(kog_sfm_core PRIVATE BLARGG_BIG_ENDIAN=1)
else()
    target_compile_definitions(kog_sfm_core PRIVATE BLARGG_LITTLE_ENDIAN=1)
endif()
if(NOT MSVC)
    # The original emulator deliberately relies on wrapping integer hardware
    # arithmetic. Preserve that behavior under current optimizing compilers.
    target_compile_options(kog_sfm_core PRIVATE -fwrapv -w)
else()
    # MSVC only implements C++ exception unwinding with /EHsc.
    target_compile_options(kog_sfm_core PRIVATE /EHsc)
endif()

add_executable(kog-sfm-helper sfm_helper.cpp)
target_compile_features(kog-sfm-helper PRIVATE cxx_std_17)
target_include_directories(kog-sfm-helper SYSTEM PRIVATE
    "${COG_GME_SFM_SOURCE}"
    "${GME}"
)
target_link_libraries(kog-sfm-helper PRIVATE kog_sfm_core)
if(MSVC)
    target_compile_options(kog-sfm-helper PRIVATE /EHsc /W4 /WX)
else()
    target_compile_options(kog-sfm-helper PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

install(TARGETS kog-sfm-helper RUNTIME DESTINATION bin)
