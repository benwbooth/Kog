/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Kog's bridge to libkss (ISC/MIT, Mitsutaka Okazaki; kmz80 public domain by
 * Mamiya), which plays MSX and Sega 8-bit KSS rips: PSG, SCC, MSX-MUSIC
 * (OPLL), MSX-AUDIO (Y8950) and the SN76489 with Game Gear stereo. Built
 * with EXCLUDE_DRIVER_ALL, so the MML driver converters are left out.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "kssplay.h"
#include "kss/kss.h"

typedef struct KogKss {
    KSS *kss;
    KSSPLAY *play;
    int16_t *pcm;
    int pcm_frames;
} KogKss;

/* Open a KSS file's bytes at raw song number `song`. */
KogKss *kog_kss_open(const uint8_t *data, size_t size, unsigned rate, int song, int master_volume,
                     char *error, size_t error_size) {
    if (size < 16 || size > 0x7fffffff) {
        snprintf(error, error_size, "not a KSS file");
        return NULL;
    }
    uint8_t *copy = malloc(size);
    if (!copy) return NULL;
    memcpy(copy, data, size);
    KSS *kss = KSS_bin2kss(copy, (uint32_t)size, "song.kss");
    free(copy);
    if (!kss) {
        snprintf(error, error_size, "libkss could not read this KSS file");
        return NULL;
    }
    KogKss *song_state = calloc(1, sizeof(KogKss));
    if (!song_state) {
        KSS_delete(kss);
        return NULL;
    }
    song_state->kss = kss;
    song_state->play = KSSPLAY_new(rate, 2, 16);
    if (!song_state->play) {
        KSS_delete(kss);
        free(song_state);
        snprintf(error, error_size, "libkss could not start");
        return NULL;
    }
    KSSPLAY_set_data(song_state->play, kss);
    KSSPLAY_reset(song_state->play, (uint32_t)(song < 0 ? 0 : song), 0);
    for (int device = 0; device < KSS_DEVICE_MAX; ++device)
        KSSPLAY_set_device_quality(song_state->play, (KSS_DEVICE)device, 1);
    KSSPLAY_set_master_volume(song_state->play, master_volume);
    return song_state;
}

/* Render `frames` stereo frames as floats. */
long kog_kss_render(KogKss *song, float *output, int frames) {
    if (song->pcm_frames < frames) {
        int16_t *grown = realloc(song->pcm, (size_t)frames * 2 * sizeof(int16_t));
        if (!grown) return -1;
        song->pcm = grown;
        song->pcm_frames = frames;
    }
    KSSPLAY_calc(song->play, song->pcm, (uint32_t)frames);
    for (int i = 0; i < frames * 2; ++i) output[i] = song->pcm[i] / 32768.0f;
    return frames;
}

/* Run the song forward without producing sound (seeking). */
void kog_kss_skip(KogKss *song, unsigned long frames) {
    while (frames > 0) {
        uint32_t step = frames > 44100 ? 44100 : (uint32_t)frames;
        KSSPLAY_calc_silent(song->play, step);
        frames -= step;
    }
}

void kog_kss_close(KogKss *song) {
    if (!song) return;
    KSSPLAY_delete(song->play);
    KSS_delete(song->kss);
    free(song->pcm);
    free(song);
}
