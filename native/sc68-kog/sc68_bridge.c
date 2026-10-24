/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Kog's bridge to sc68 (GPL-3.0-or-later, Benjamin Gerard), which emulates
 * the Atari ST's 68000, YM2149, MFP timers and STE DMA sound to play SNDH and
 * sc68 files. No config file is read or written and nothing touches disk.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>

#include "sc68/sc68.h"

static pthread_once_t once = PTHREAD_ONCE_INIT;
static int init_result = -1;

static void quiet(const int bit, sc68_t *sc68, const char *fmt, va_list list) {
    (void)bit; (void)sc68; (void)fmt; (void)list;
}

static void initialize(void) {
    sc68_init_t init;
    memset(&init, 0, sizeof(init));
    init.msg_handler = (sc68_msg_t)quiet;
    init.flags.no_load_config = 1;
    init.flags.no_save_config = 1;
    init_result = sc68_init(&init);
}

typedef struct KogSc68 {
    sc68_t *sc68;
    int tracks;
    int track;
    unsigned time_ms;
    char title[256], artist[256], album[256], hardware[64];
    short *pcm;
    int pcm_frames;
    char error[256];
} KogSc68;

static void copy_text(char *out, size_t size, const char *text) {
    snprintf(out, size, "%s", text ? text : "");
}

/* Open a file's bytes at `track` (0-based). Returns NULL with a message. */
KogSc68 *kog_sc68_open(const void *data, size_t size, unsigned rate, int track,
                       char *error, size_t error_size) {
    pthread_once(&once, initialize);
    if (init_result != 0) {
        copy_text(error, error_size, "sc68 could not start");
        return NULL;
    }
    sc68_create_t create;
    memset(&create, 0, sizeof(create));
    create.sampling_rate = rate;
    create.name = "kog";
    KogSc68 *song = calloc(1, sizeof(KogSc68));
    if (!song) return NULL;
    song->sc68 = sc68_create(&create);
    if (!song->sc68 || sc68_load_mem(song->sc68, data, (int)size) != 0) {
        const char *why = song->sc68 ? sc68_error(song->sc68) : NULL;
        copy_text(error, error_size, why && *why ? why : "sc68 could not load this file");
        if (song->sc68) sc68_destroy(song->sc68);
        free(song);
        return NULL;
    }
    sc68_music_info_t disk;
    memset(&disk, 0, sizeof(disk));
    sc68_music_info(song->sc68, &disk, SC68_DEF_TRACK, 0);
    song->tracks = disk.tracks > 0 ? disk.tracks : 1;
    song->track = track < 0 || track >= song->tracks ? 0 : track;
    /* Loop 1: play the track once; Kog fades it out itself. */
    if (sc68_play(song->sc68, song->track + 1, 1) != 0) {
        copy_text(error, error_size, "sc68 could not start the track");
        sc68_destroy(song->sc68);
        free(song);
        return NULL;
    }
    sc68_music_info_t info;
    memset(&info, 0, sizeof(info));
    sc68_music_info(song->sc68, &info, song->track + 1, 0);
    song->time_ms = info.trk.time_ms;
    copy_text(song->title, sizeof(song->title), info.title);
    copy_text(song->artist, sizeof(song->artist), info.artist);
    copy_text(song->album, sizeof(song->album), info.album);
    copy_text(song->hardware, sizeof(song->hardware), info.trk.hw);
    return song;
}

const char *kog_sc68_last_error(const KogSc68 *song) { return song->error; }

int kog_sc68_tracks(const KogSc68 *song) { return song->tracks; }
unsigned kog_sc68_time_ms(const KogSc68 *song) { return song->time_ms; }
const char *kog_sc68_text(const KogSc68 *song, int which) {
    switch (which) {
    case 0: return song->title;
    case 1: return song->artist;
    case 2: return song->album;
    default: return song->hardware;
    }
}

/* Render up to `frames` stereo frames as floats; returns how many, 0 at the
 * end of the track, -1 on an emulation error. */
long kog_sc68_render(KogSc68 *song, float *output, int frames) {
    if (song->pcm_frames < frames) {
        short *grown = realloc(song->pcm, (size_t)frames * 2 * sizeof(short));
        if (!grown) return -1;
        song->pcm = grown;
        song->pcm_frames = frames;
    }
    int n = 0;
    /* A pass can produce nothing (the track just started or changed). */
    for (int pass = 0; pass < 64 && n == 0; ++pass) {
        n = frames;
        const int code = sc68_process(song->sc68, song->pcm, &n);
        if (code == SC68_ERROR) {
            snprintf(song->error, sizeof(song->error), "%s", sc68_error(song->sc68) ? sc68_error(song->sc68) : "sc68 failed");
            return -1;
        }
        if (n == 0 && (code & SC68_END)) return 0;
    }
    for (int i = 0; i < n * 2; ++i) output[i] = song->pcm[i] / 32768.0f;
    return n;
}

void kog_sc68_close(KogSc68 *song) {
    if (!song) return;
    if (song->sc68) sc68_destroy(song->sc68);
    free(song->pcm);
    free(song);
}
