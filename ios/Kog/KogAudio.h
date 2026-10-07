#ifndef KOG_IOS_AUDIO_H
#define KOG_IOS_AUDIO_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct KogAudioHandle KogAudioHandle;

KogAudioHandle *kog_audio_open(const char *path, int32_t subsong, const char *midi_engine,
                              const char *soundfont_path, const char *sc55_rom_path,
                              const char *mt32_rom_path, char *error, size_t error_capacity);
KogAudioHandle *kog_audio_open_stream(const char *url, const char *headers, uint64_t duration_ms,
                                     char *error, size_t error_capacity);
// Context ownership transfers even on failure. The decoder calls close once.
// read returns a byte count, zero for EOF, or a negative value on failure.
typedef int32_t (*KogAudioStreamRead)(void *context, uint8_t *output, int32_t capacity);
typedef void (*KogAudioStreamClose)(void *context);
KogAudioHandle *kog_audio_open_reader(KogAudioStreamRead read, KogAudioStreamClose close,
                                     void *context, uint64_t duration_ms,
                                     char *error, size_t error_capacity);
// Free the JSON result with kog_audio_string_free. Position is device-consumed time.
char *kog_audio_channel_snapshot(const KogAudioHandle *handle, uint64_t position_ms, bool playing);
int64_t kog_audio_duration_ms(const KogAudioHandle *handle);
intptr_t kog_audio_read(KogAudioHandle *handle, uint8_t *output, size_t capacity,
                        char *error, size_t error_capacity);
bool kog_audio_seek(KogAudioHandle *handle, uint64_t position_ms, char *error, size_t error_capacity);
void kog_audio_close(KogAudioHandle *handle);

// JSON library operations shared with the server. Free with kog_audio_string_free.
char *kog_library_request(const char *request, char *error, size_t error_capacity);
char *kog_preferences_request(const char *request, char *error, size_t error_capacity);
char *kog_audio_browse(const char *root, const char *path, char *error, size_t error_capacity);
char *kog_audio_expand(const char *path, char *error, size_t error_capacity);
void kog_audio_string_free(char *json);
uint8_t *kog_audio_artwork(const char *path, size_t *length);
void kog_audio_bytes_free(uint8_t *bytes, size_t length);

#endif
