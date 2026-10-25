/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Kog's bridge to libkss (ISC/MIT, Mitsutaka Okazaki; kmz80 public domain by
 * Mamiya), which plays MSX and Sega 8-bit KSS rips: PSG, SCC, MSX-MUSIC
 * (OPLL), MSX-AUDIO (Y8950) and the SN76489 with Game Gear stereo. Built
 * with EXCLUDE_DRIVER_ALL, so the MML driver converters are left out.
 */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "kssplay.h"
#include "kss/kss.h"
#include "inspection.h"

/* What the song has written to each chip, for the channel inspector. The
 * writes are watched on their way to libkss's own chips. */
typedef struct KogKssChips {
    uint8_t psg[16], psg_latch;
    uint8_t scc[16];           /* frequency 0-9, volume 10-14, enable 15 */
    uint8_t opll[64], opll_latch;
    uint8_t opl[256], opl_latch;
    uint16_t sn[8];            /* period/control and attenuation per channel */
    uint8_t sn_latch, gg_stereo;
    unsigned written;          /* bit per chip: PSG, SCC, OPLL, OPL, SN */
} KogKssChips;

typedef struct KogKss {
    KSS *kss;
    KSSPLAY *play;
    int16_t *pcm;
    int pcm_frames;
    KogKssChips chips;
} KogKss;

static void watch_io(void *context, uint32_t a, uint32_t d) {
    KogKssChips *c = context;
    d &= 0xff;
    switch (a & 0xff) {
    case 0xa0: c->psg_latch = d & 15; break;
    case 0xa1: c->psg[c->psg_latch] = d; c->written |= 1; break;
    case 0x7c: case 0xf0: c->opll_latch = d & 63; break;
    case 0x7d: case 0xf1: c->opll[c->opll_latch] = d; c->written |= 4; break;
    case 0xc0: c->opl_latch = d; break;
    case 0xc1: c->opl[c->opl_latch] = d; c->written |= 8; break;
    case 0x06: c->gg_stereo = d; break;
    case 0x7e: case 0x7f:
        if (d & 0x80) {
            c->sn_latch = (d >> 4) & 7;
            c->sn[c->sn_latch] = (c->sn[c->sn_latch] & 0x3f0) | (d & 15);
        } else if (!(c->sn_latch & 1) && c->sn_latch < 6) {
            c->sn[c->sn_latch] = (c->sn[c->sn_latch] & 15) | ((d & 63) << 4);
        } else {
            c->sn[c->sn_latch] = d & 15;
        }
        c->written |= 16;
        break;
    }
}

static void watch_memory(void *context, uint32_t a, uint32_t d) {
    KogKssChips *c = context;
    /* SCC registers at 9880-988F (mirrored at 9890-989F); SCC-I at B8A0-B8AF
     * (mirrored at B8B0-B8BF). The rest is waveform memory. */
    const unsigned low = a & 0xff;
    if ((a & 0xff00) == 0x9800 && low >= 0x80 && low < 0xa0) c->scc[low & 15] = (uint8_t)d;
    else if ((a & 0xff00) == 0xb800 && low >= 0xa0 && low < 0xc0) c->scc[low & 15] = (uint8_t)d;
    else return;
    c->written |= 2;
}

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
    memset(&song_state->chips, 0, sizeof(song_state->chips));
    song_state->chips.gg_stereo = 0xff;
    KSSPLAY_set_iowrite_handler(song_state->play, &song_state->chips, watch_io);
    KSSPLAY_set_memwrite_handler(song_state->play, &song_state->chips, watch_memory);
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

/* ---- Channel inspector ---- */

static float frequency_key(double hz) {
    return hz > 0.0 && isfinite(hz) ? (float)(69.0 + 12.0 * log2(hz / 440.0)) : -1.0f;
}

typedef struct VoiceList {
    KogVoice *out;
    size_t capacity, count;
} VoiceList;

static KogVoice *add_voice(VoiceList *list, uint32_t id, const char *name, uint32_t kind, int active, double hz, float level, float pan) {
    static KogVoice discarded;
    if (list->count >= list->capacity) return &discarded;
    KogVoice *v = &list->out[list->count++];
    memset(v, 0, sizeof(*v));
    v->id = id;
    v->kind = kind;
    v->active = active ? 1 : 0;
    v->key = kind == 0 ? frequency_key(hz) : -1.0f;
    v->level = level < 0 ? 0 : level > 1 ? 1 : level;
    v->pan = pan;
    snprintf(v->name, sizeof(v->name), "%s", name);
    return v;
}

#define MSX_CLOCK 3579545.0

/* Voices for every chip the song has written to: their programmed tone,
 * key gates and levels (not the chips' envelope output). A zero period has
 * no meaningful pitch, so it gets no key. */
size_t kog_kss_inspection(KogKss *song, KogVoice *voices, size_t capacity) {
    const KogKssChips *c = &song->chips;
    VoiceList list = { voices, capacity, 0 };
    char name[64];
    if (c->written & 16) {
        for (unsigned ch = 0; ch < 4; ++ch) {
            const unsigned period = c->sn[ch * 2], attenuation = c->sn[ch * 2 + 1] & 15;
            const int right = (c->gg_stereo >> ch) & 1, left = (c->gg_stereo >> (ch + 4)) & 1;
            snprintf(name, sizeof(name), ch < 3 ? "SN76489 Tone %u" : "SN76489 Noise", ch + 1);
            KogVoice *v = add_voice(&list, 0x400 + ch, name, ch == 3 ? 1 : 0, attenuation < 15 && (left || right),
                ch < 3 && period ? MSX_CLOCK / (32.0 * period) : 0, powf(10.0f, -(float)attenuation / 10.0f), (float)(right - left));
            if (ch == 3)
                snprintf(v->instrument, sizeof(v->instrument), "%s noise, %s", (period & 4) ? "White" : "Periodic",
                    (period & 3) == 3 ? "rate from tone 3" : (period & 3) == 2 ? "low rate" : (period & 3) == 1 ? "middle rate" : "high rate");
            snprintf(v->details, sizeof(v->details), "Period/control=%03X | Attenuation=%u | Game Gear stereo=%02X", period, attenuation, c->gg_stereo);
        }
    }
    if (c->written & 1) {
        for (unsigned ch = 0; ch < 3; ++ch) {
            const unsigned period = c->psg[ch * 2] | ((c->psg[ch * 2 + 1] & 15) << 8);
            const int tone = !(c->psg[7] & (1 << ch)), noise = !(c->psg[7] & (8 << ch));
            const unsigned volume = c->psg[8 + ch];
            snprintf(name, sizeof(name), "MSX PSG %c", 'A' + ch);
            KogVoice *v = add_voice(&list, 0x100 + ch, name, tone ? 0 : 1, (tone || noise) && (volume & 31),
                tone && period ? MSX_CLOCK / 2.0 / (16.0 * period) : 0, (volume & 16) ? 1.0f : (volume & 15) / 15.0f, 0);
            snprintf(v->instrument, sizeof(v->instrument), "%s%s%s", tone ? "Tone" : "", noise ? (tone ? " + noise" : "Noise") : "", (volume & 16) ? " + envelope" : "");
            snprintf(v->details, sizeof(v->details), "Period=%03X | Volume=%02X | Mixer=%02X | Noise period=%02X | Envelope=%02X%02X shape %X",
                period, volume, c->psg[7], c->psg[6] & 31, c->psg[12], c->psg[11], c->psg[13] & 15);
        }
    }
    if (c->written & 2) {
        for (unsigned ch = 0; ch < 5; ++ch) {
            const unsigned period = c->scc[ch * 2] | ((c->scc[ch * 2 + 1] & 15) << 8);
            const unsigned volume = c->scc[10 + ch] & 15;
            snprintf(name, sizeof(name), "SCC Wave %u", ch + 1);
            KogVoice *v = add_voice(&list, 0x200 + ch, name, 0, (c->scc[15] & (1 << ch)) && volume,
                period ? MSX_CLOCK / (32.0 * (period + 1)) : 0, volume / 15.0f, 0);
            snprintf(v->details, sizeof(v->details), "Period=%03X | Volume=%X | Enable=%02X", period, volume, c->scc[15]);
        }
    }
    if (c->written & 4) {
        static const char *const drums[5] = { "Hi-hat", "Cymbal", "Tom", "Snare", "Bass drum" };
        const int rhythm = c->opll[0x0e] & 0x20;
        for (unsigned ch = 0; ch < (rhythm ? 6u : 9u); ++ch) {
            const unsigned fnum = c->opll[0x10 + ch] | ((c->opll[0x20 + ch] & 1) << 8), block = (c->opll[0x20 + ch] >> 1) & 7;
            snprintf(name, sizeof(name), "MSX-MUSIC FM %u", ch + 1);
            KogVoice *v = add_voice(&list, 0x300 + ch, name, 0, c->opll[0x20 + ch] & 16,
                MSX_CLOCK / 72.0 * fnum * (1 << block) / 524288.0, powf(10.0f, -(c->opll[0x30 + ch] & 15) * 3.0f / 20.0f), 0);
            snprintf(v->instrument, sizeof(v->instrument), "%s", (c->opll[0x30 + ch] >> 4) ? "" : "Custom instrument");
            if (c->opll[0x30 + ch] >> 4) snprintf(v->instrument, sizeof(v->instrument), "OPLL instrument %u", c->opll[0x30 + ch] >> 4);
            snprintf(v->details, sizeof(v->details), "F-number=%03X | Block=%u | Key/sustain=%02X | Instrument/volume=%02X",
                fnum, block, c->opll[0x20 + ch], c->opll[0x30 + ch]);
        }
        if (rhythm)
            for (unsigned drum = 0; drum < 5; ++drum) {
                snprintf(name, sizeof(name), "MSX-MUSIC %s", drums[drum]);
                KogVoice *v = add_voice(&list, 0x310 + drum, name, 3, c->opll[0x0e] & (1 << drum), 0, 1.0f, 0);
                snprintf(v->instrument, sizeof(v->instrument), "Rhythm");
                snprintf(v->details, sizeof(v->details), "Rhythm=%02X", c->opll[0x0e]);
            }
    }
    if (c->written & 8) {
        static const int slots[9] = { 0, 1, 2, 8, 9, 10, 16, 17, 18 };
        static const char *const drums[5] = { "Hi-hat", "Cymbal", "Tom", "Snare", "Bass drum" };
        const int rhythm = c->opl[0xbd] & 0x20;
        for (unsigned ch = 0; ch < (rhythm ? 6u : 9u); ++ch) {
            const unsigned fnum = c->opl[0xa0 + ch] | ((c->opl[0xb0 + ch] & 3) << 8), block = (c->opl[0xb0 + ch] >> 2) & 7;
            const unsigned carrier = slots[ch] + 3, tl = c->opl[0x40 + carrier] & 63;
            snprintf(name, sizeof(name), "MSX-AUDIO FM %u", ch + 1);
            KogVoice *v = add_voice(&list, 0x500 + ch, name, 0, c->opl[0xb0 + ch] & 0x20,
                MSX_CLOCK / 72.0 * fnum * (1 << block) / 1048576.0, powf(10.0f, -0.75f * tl / 20.0f), 0);
            snprintf(v->instrument, sizeof(v->instrument), "2-op FM");
            snprintf(v->details, sizeof(v->details), "F-number=%03X | Block=%u | Key/pitch=%02X | Algorithm/feedback=%02X | Carrier level=%02X",
                fnum, block, c->opl[0xb0 + ch], c->opl[0xc0 + ch], c->opl[0x40 + carrier]);
        }
        if (rhythm)
            for (unsigned drum = 0; drum < 5; ++drum) {
                snprintf(name, sizeof(name), "MSX-AUDIO %s", drums[drum]);
                KogVoice *v = add_voice(&list, 0x510 + drum, name, 3, c->opl[0xbd] & (1 << drum), 0, 1.0f, 0);
                snprintf(v->instrument, sizeof(v->instrument), "Rhythm");
                snprintf(v->details, sizeof(v->details), "Rhythm=%02X", c->opl[0xbd]);
            }
        KogVoice *v = add_voice(&list, 0x520, "MSX-AUDIO ADPCM", 2, c->opl[7] & 0x80, 0, c->opl[0x12] / 255.0f, 0);
        snprintf(v->instrument, sizeof(v->instrument), "ADPCM");
        snprintf(v->details, sizeof(v->details), "Control=%02X | Rate=%02X%02X | Level=%02X", c->opl[7], c->opl[0x11], c->opl[0x10], c->opl[0x12]);
    }
    return list.count;
}
