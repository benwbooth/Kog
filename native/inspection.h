#ifndef KOG_INSPECTION_H
#define KOG_INSPECTION_H
#include <stddef.h>
#include <stdint.h>

// Fixed-size, pointer-free snapshot shared by the native renderers and Rust.
// A negative key means the renderer cannot establish a musical note.
typedef struct KogVoice {
    uint32_t id;
    uint32_t kind; // 0 tonal, 1 noise, 2 sample, 3 percussion, 4 mixed
    uint32_t active;
    float key;
    float level;
    float pan;
    char name[64];
    char instrument[64];
    char details[512];
} KogVoice;

#ifdef __cplusplus
extern "C" {
#else
#include <stdbool.h>
#endif
#ifdef KOG_EMBEDDED
bool kog_inspection_enabled(void);
void kog_inspection_publish(double position, const KogVoice *voices, size_t count);
#else
#include <stdio.h>
#include <stdlib.h>
// Standalone Windows renderers share a bounded file ring with their parent.
// PCM keeps its existing pipe protocol. A slot is accepted only when its two
// sequence markers agree, so a concurrent overwrite cannot publish torn data.
static inline FILE *kog_inspection_file(void) {
    static FILE *file = NULL;
    static bool opened = false;
    if (!opened) {
        opened = true;
        const char *path = getenv("KOG_CHANNEL_RING");
        if (path && *path) file = fopen(path, "r+b");
    }
    return file;
}
static inline bool kog_inspection_enabled(void) { return kog_inspection_file() != NULL; }
static inline void kog_inspection_publish(double position, const KogVoice *voices, size_t count) {
    FILE *file = kog_inspection_file();
    static uint64_t sequence = 0;
    if (!file || count > 64) return;
    const uint64_t serial = ++sequence, zero = 0;
    const uint32_t length = (uint32_t)count, reserved = 0;
    const long slot_size = 32 + 64 * (long)sizeof(KogVoice);
    const long offset = 8 + (long)((serial - 1) % 256) * slot_size;
    fseek(file, offset, SEEK_SET); fwrite(&zero, 8, 1, file);
    fwrite(&position, 8, 1, file); fwrite(&length, 4, 1, file); fwrite(&reserved, 4, 1, file);
    fwrite(voices, sizeof(KogVoice), count, file);
    fseek(file, offset + slot_size - 8, SEEK_SET); fwrite(&serial, 8, 1, file);
    fseek(file, offset, SEEK_SET); fwrite(&serial, 8, 1, file);
    fseek(file, 0, SEEK_SET); fwrite(&serial, 8, 1, file); fflush(file);
}
#endif
#ifdef __cplusplus
}
#endif

#ifdef __cplusplus
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstring>

inline float kog_frequency_key(double hz) {
    return hz > 0.0 && std::isfinite(hz) ? static_cast<float>(69.0 + 12.0 * std::log2(hz / 440.0)) : -1.0f;
}

struct KogVoices {
    KogVoice *out;
    size_t capacity;
    size_t count = 0;
    KogVoice discarded{};
    KogVoice &add(const char *name, uint32_t kind, bool active, double hz, float level, float pan = 0.0f) {
        auto &v = count < capacity ? out[count] : discarded;
        v = {};
        v.id = static_cast<uint32_t>(count);
        if (count < capacity) ++count;
        v.kind = kind;
        v.active = active;
        v.key = kog_frequency_key(hz);
        v.level = active ? std::clamp(level, 0.0f, 1.0f) : 0.0f;
        v.pan = std::clamp(pan, -1.0f, 1.0f);
        std::snprintf(v.name, sizeof(v.name), "%s", name);
        return v;
    }
};
#endif
#endif
