#ifndef KOG_FFMPEG_ENCODER_BRIDGE_H
#define KOG_FFMPEG_ENCODER_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int (*KogFfmpegWrite)(void *opaque, const uint8_t *bytes, int count);

// codec: 0 = AAC/ADTS, 1 = Opus/Ogg, 2 = FLAC.
void *kog_ffmpeg_encoder_open(int codec, int bitrate_kbps, int input_rate,
                              int channels, KogFfmpegWrite write, void *opaque,
                              char *error, size_t error_capacity);
int kog_ffmpeg_encoder_push(void *encoder, const float *interleaved, int frames);
int kog_ffmpeg_encoder_finish(void *encoder);
const char *kog_ffmpeg_encoder_error(const void *encoder);
void kog_ffmpeg_encoder_close(void *encoder);

#ifdef __cplusplus
}
#endif
#endif
