#ifndef KOG_LIBVGM_H
#define KOG_LIBVGM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct kog_libvgm kog_libvgm;

kog_libvgm *kog_libvgm_create(
    const uint8_t *data,
    size_t data_size,
    const uint8_t *yrw801_rom,
    size_t yrw801_rom_size,
    uint32_t sample_rate,
    uint32_t loop_count,
    uint32_t fade_samples,
    uint32_t end_silence_samples,
    char *error,
    size_t error_size);

void kog_libvgm_destroy(kog_libvgm *decoder);
uint64_t kog_libvgm_total_frames(const kog_libvgm *decoder);
const char *kog_libvgm_title(const kog_libvgm *decoder);
const char *kog_libvgm_artist(const kog_libvgm *decoder);
const char *kog_libvgm_album(const kog_libvgm *decoder);
const char *kog_libvgm_date(const kog_libvgm *decoder);
const char *kog_libvgm_codec(const kog_libvgm *decoder);
const char *kog_libvgm_warning(const kog_libvgm *decoder);
const char *kog_libvgm_last_error(const kog_libvgm *decoder);
size_t kog_libvgm_render(kog_libvgm *decoder, float *output, size_t frames);
int kog_libvgm_seek(kog_libvgm *decoder, uint64_t frame);

#ifdef __cplusplus
}
#endif

#endif
