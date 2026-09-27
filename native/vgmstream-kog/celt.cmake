# FMOD used two incompatible CELT bitstreams. Both must coexist with FFmpeg,
# Vorbis and Speex, so use upstream vgmstream's symbol namespace plus the FFT
# and helper symbols that can otherwise collide in a single static executable.
file(READ "${VGM}/cmake/dependencies/celt.cmake" upstream_celt)
string(REGEX MATCH "set\\(CELT_CFLAGS([^)]*)\\)" unused "${upstream_celt}")
string(REGEX MATCHALL "[A-Za-z_][A-Za-z_0-9]*" celt_symbols "${CMAKE_MATCH_1}")
list(APPEND celt_symbols celt_header_init celt_header_to_packet celt_header_from_packet
  celt_encoder_create_custom
  celt_strerror celt_get_version_string find_spectral_pitch compute_pitch_gain
  pitch_downsample pitch_search remove_doubling comb_filter fir iir
  kiss_fft_alloc_float kiss_fft_float kiss_ifft_float kiss_fft_stride_float
  kiss_ifft_stride_float kiss_fft_alloc_twiddles_float kiss_fft_free_float)
foreach(version 0061 0110)
  set(source "${NATIVE}/celt-${version}/libcelt")
  file(READ "${source}/Makefile.am" manifest)
  string(REPLACE "\\\n" " " manifest "${manifest}")
  string(REGEX MATCH "_la_SOURCES =[^\n]*" source_line "${manifest}")
  string(REGEX MATCHALL "[a-z_0-9]+\\.c" sources "${source_line}")
  list(TRANSFORM sources PREPEND "${source}/")
  add_library(kog_celt${version} STATIC ${sources})
  target_include_directories(kog_celt${version} PRIVATE "${source}")
  target_compile_definitions(kog_celt${version} PRIVATE FLOATING_POINT USE_ALLOCA
    CUSTOM_MODES CELT_BUILD HAVE_LRINT HAVE_LRINTF)
  foreach(symbol IN LISTS celt_symbols)
    target_compile_definitions(kog_celt${version} PRIVATE "${symbol}=${symbol}_${version}")
  endforeach()
endforeach()
