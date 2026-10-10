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
// File formats: 3 = WAV 16-bit, 4 = WAV 24-bit, 5 = FLAC 24-bit, 6 = ALAC/M4A,
// 7 = MP3, 8 = AAC/M4A, 9 = Vorbis/Ogg, 10 = Opus/Ogg. `tags` holds
// key/value pairs and ends with a NULL key.
void *kog_ffmpeg_encoder_open_file(int format, int bitrate_kbps, int input_rate,
                                   int channels, const char *path,
                                   const char *const *tags, char *error,
                                   size_t error_capacity);
int kog_ffmpeg_encoder_push(void *encoder, const float *interleaved, int frames);
int kog_ffmpeg_encoder_finish(void *encoder);
const char *kog_ffmpeg_encoder_error(const void *encoder);
void kog_ffmpeg_encoder_close(void *encoder);

#ifdef __cplusplus
}
#endif
#endif
