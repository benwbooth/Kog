// Kog C ABI wrapper for Cog's OPL3Windows MIDI synthesizer.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct kog_opl3w kog_opl3w;

kog_opl3w *kog_opl3w_create(uint32_t sample_rate);
void kog_opl3w_destroy(kog_opl3w *synth);
void kog_opl3w_write(kog_opl3w *synth, uint32_t packed_midi);
void kog_opl3w_generate(kog_opl3w *synth, int16_t *stereo, uint32_t frames);

#ifdef __cplusplus
}
#endif
