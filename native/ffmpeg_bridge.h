#ifndef KOG_FFMPEG_BRIDGE_H
#define KOG_FFMPEG_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogFfmpeg KogFfmpeg;

KogFfmpeg *kog_ffmpeg_open(const char *path);
void kog_ffmpeg_close(KogFfmpeg *decoder);

const char *kog_ffmpeg_error(const KogFfmpeg *decoder);
const char *kog_ffmpeg_version(void);
const char *kog_ffmpeg_codec(const KogFfmpeg *decoder);
const char *kog_ffmpeg_title(const KogFfmpeg *decoder);
const char *kog_ffmpeg_artist(const KogFfmpeg *decoder);
const char *kog_ffmpeg_album(const KogFfmpeg *decoder);
const char *kog_ffmpeg_genre(const KogFfmpeg *decoder);
const char *kog_ffmpeg_cuesheet(const KogFfmpeg *decoder);

uint32_t kog_ffmpeg_sample_rate(const KogFfmpeg *decoder);
uint16_t kog_ffmpeg_channels(const KogFfmpeg *decoder);
uint32_t kog_ffmpeg_bitrate(const KogFfmpeg *decoder);
uint8_t kog_ffmpeg_bits_per_sample(const KogFfmpeg *decoder);
uint32_t kog_ffmpeg_year(const KogFfmpeg *decoder);
uint32_t kog_ffmpeg_track(const KogFfmpeg *decoder);
double kog_ffmpeg_duration(const KogFfmpeg *decoder);

// Returns decoded sample frames, zero at end of stream, or -1 on error.
int32_t kog_ffmpeg_render(KogFfmpeg *decoder, float *output, uint32_t frames);
int32_t kog_ffmpeg_seek(KogFfmpeg *decoder, double seconds);

#ifdef __cplusplus
}
#endif

#endif
