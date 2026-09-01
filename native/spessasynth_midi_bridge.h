// Kog C ABI adapter for SpessaSynth Core C's MIDI container parser.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum kog_spessasynth_midi_result {
    KOG_SPESSASYNTH_MIDI_OK = 0,
    KOG_SPESSASYNTH_MIDI_INVALID_ARGUMENT = 1,
    KOG_SPESSASYNTH_MIDI_OPEN_FAILED = 2,
    KOG_SPESSASYNTH_MIDI_PARSE_FAILED = 3,
    KOG_SPESSASYNTH_MIDI_WRITE_FAILED = 4,
    KOG_SPESSASYNTH_MIDI_OUTPUT_TOO_LARGE = 5,
    KOG_SPESSASYNTH_MIDI_ALLOCATION_FAILED = 6,
};

int kog_spessasynth_midi_convert(const uint8_t *input,
                                 size_t input_size,
                                 const char *file_name,
                                 uint8_t **midi_data,
                                 size_t *midi_size,
                                 uint8_t **title_data,
                                 size_t *title_size);

void kog_spessasynth_midi_free(void *data);

#ifdef __cplusplus
}
#endif
