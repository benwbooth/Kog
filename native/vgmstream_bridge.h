#ifndef KOG_VGMSTREAM_BRIDGE_H
#define KOG_VGMSTREAM_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogVgmstream KogVgmstream;

enum KogVgmstreamError {
    KOG_VGMSTREAM_OK = 0,
    KOG_VGMSTREAM_INVALID_ARGUMENT = 1,
    KOG_VGMSTREAM_OPEN_FAILED = 2,
    KOG_VGMSTREAM_DECODE_FAILED = 3,
};

KogVgmstream *kog_vgmstream_open(
    const char *path,
    int32_t subsong,
    double loop_count,
    double fade_seconds,
    int *error
);
void kog_vgmstream_free(KogVgmstream *decoder);

uint32_t kog_vgmstream_sample_rate(const KogVgmstream *decoder);
uint32_t kog_vgmstream_channels(const KogVgmstream *decoder);
uint64_t kog_vgmstream_total_frames(const KogVgmstream *decoder);
uint32_t kog_vgmstream_subsong_count(const KogVgmstream *decoder);
uint32_t kog_vgmstream_selected_subsong(const KogVgmstream *decoder);
uint32_t kog_vgmstream_bitrate(const KogVgmstream *decoder);
const char *kog_vgmstream_codec(const KogVgmstream *decoder);
const char *kog_vgmstream_title(const KogVgmstream *decoder);
const char *kog_vgmstream_artist(const KogVgmstream *decoder);
const char *kog_vgmstream_album(const KogVgmstream *decoder);
uint32_t kog_vgmstream_year(const KogVgmstream *decoder);
uint32_t kog_vgmstream_track_number(const KogVgmstream *decoder);

int64_t kog_vgmstream_render(KogVgmstream *decoder, float *output, size_t frames);
uint64_t kog_vgmstream_seek(KogVgmstream *decoder, uint64_t frame);

int kog_vgmstream_supports_extension(const char *extension);
size_t kog_vgmstream_extension_count(void);
const char *kog_vgmstream_extension(size_t index);
uint32_t kog_vgmstream_api_version(void);

#ifdef __cplusplus
}
#endif

#endif
