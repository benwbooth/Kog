/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Kog's bridge to ayumi (MIT), an AY-3-8910/YM2149 emulator, for YM register
 * dumps from the Atari ST, Amstrad CPC and ZX Spectrum.
 */
#include <stdlib.h>
#include <stdint.h>
#include "ayumi/ayumi.h"

typedef struct KogAyumi {
    struct ayumi ay;
} KogAyumi;

KogAyumi *kog_ayumi_new(int is_ym, double clock_rate, int sample_rate, int stereo) {
    KogAyumi *chip = calloc(1, sizeof(KogAyumi));
    if (!chip) return NULL;
    if (!ayumi_configure(&chip->ay, is_ym, clock_rate, sample_rate)) {
        free(chip);
        return NULL;
    }
    /* The ST's YM is mono; CPC and Spectrum rips sound best as ABC stereo. */
    const double pans[3] = {stereo ? 0.1 : 0.5, 0.5, stereo ? 0.9 : 0.5};
    for (int index = 0; index < 3; ++index) ayumi_set_pan(&chip->ay, index, pans[index], 0);
    return chip;
}

void kog_ayumi_free(KogAyumi *chip) { free(chip); }

/* Load one 50 Hz frame of the 14 sound registers. A 0xff envelope shape
 * means "unchanged": writing it would restart the envelope. */
void kog_ayumi_set_registers(KogAyumi *chip, const uint8_t *r) {
    struct ayumi *ay = &chip->ay;
    ayumi_set_tone(ay, 0, ((r[1] & 0x0f) << 8) | r[0]);
    ayumi_set_tone(ay, 1, ((r[3] & 0x0f) << 8) | r[2]);
    ayumi_set_tone(ay, 2, ((r[5] & 0x0f) << 8) | r[4]);
    ayumi_set_noise(ay, r[6] & 0x1f);
    ayumi_set_mixer(ay, 0, r[7] & 1, (r[7] >> 3) & 1, (r[8] >> 4) & 1);
    ayumi_set_mixer(ay, 1, (r[7] >> 1) & 1, (r[7] >> 4) & 1, (r[9] >> 4) & 1);
    ayumi_set_mixer(ay, 2, (r[7] >> 2) & 1, (r[7] >> 5) & 1, (r[10] >> 4) & 1);
    ayumi_set_volume(ay, 0, r[8] & 0x0f);
    ayumi_set_volume(ay, 1, r[9] & 0x0f);
    ayumi_set_volume(ay, 2, r[10] & 0x0f);
    ayumi_set_envelope(ay, (r[12] << 8) | r[11]);
    if (r[13] != 0xff) ayumi_set_envelope_shape(ay, r[13] & 0x0f);
}

/* Render interleaved stereo frames. */
void kog_ayumi_render(KogAyumi *chip, float *output, size_t frames) {
    for (size_t frame = 0; frame < frames; ++frame) {
        ayumi_process(&chip->ay);
        ayumi_remove_dc(&chip->ay);
        /* Three full-volume channels sum to about 1.5; leave headroom. */
        output[frame * 2] = (float)(chip->ay.left * 0.6);
        output[frame * 2 + 1] = (float)(chip->ay.right * 0.6);
    }
}
