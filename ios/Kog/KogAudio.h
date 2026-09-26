#ifndef KOG_IOS_AUDIO_H
#define KOG_IOS_AUDIO_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct KogAudioHandle KogAudioHandle;

KogAudioHandle *kog_audio_open(const char *path, int32_t subsong, char *error, size_t error_capacity);
int64_t kog_audio_duration_ms(const KogAudioHandle *handle);
intptr_t kog_audio_read(KogAudioHandle *handle, uint8_t *output, size_t capacity,
                        char *error, size_t error_capacity);
bool kog_audio_seek(KogAudioHandle *handle, uint64_t position_ms, char *error, size_t error_capacity);
void kog_audio_close(KogAudioHandle *handle);

char *kog_audio_browse(const char *root, const char *path, char *error, size_t error_capacity);
char *kog_audio_expand(const char *path, char *error, size_t error_capacity);
void kog_audio_string_free(char *json);
uint8_t *kog_audio_artwork(const char *path, size_t *length);
void kog_audio_bytes_free(uint8_t *bytes, size_t length);

#endif
