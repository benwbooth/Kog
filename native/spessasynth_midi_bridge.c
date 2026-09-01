// Kog C ABI adapter for SpessaSynth Core C's MIDI container parser.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#include "spessasynth_midi_bridge.h"

#include <spessasynth/midi/midi.h>

#include <stdlib.h>
#include <string.h>

#define KOG_SPESSASYNTH_MAX_OUTPUT (256u * 1024u * 1024u)

static int copy_bytes(const uint8_t *source, size_t size, uint8_t **output) {
    if (size == 0) {
        *output = NULL;
        return KOG_SPESSASYNTH_MIDI_OK;
    }
    uint8_t *copy = (uint8_t *)malloc(size);
    if (!copy) {
        return KOG_SPESSASYNTH_MIDI_ALLOCATION_FAILED;
    }
    memcpy(copy, source, size);
    *output = copy;
    return KOG_SPESSASYNTH_MIDI_OK;
}

int kog_spessasynth_midi_convert(const uint8_t *input,
                                 size_t input_size,
                                 const char *file_name,
                                 uint8_t **midi_data,
                                 size_t *midi_size,
                                 uint8_t **title_data,
                                 size_t *title_size) {
    if (!input || input_size == 0 || !midi_data || !midi_size ||
        !title_data || !title_size) {
        return KOG_SPESSASYNTH_MIDI_INVALID_ARGUMENT;
    }
    *midi_data = NULL;
    *midi_size = 0;
    *title_data = NULL;
    *title_size = 0;

    SS_File *input_file = ss_file_open_from_memory(input, input_size, false);
    if (!input_file) {
        return KOG_SPESSASYNTH_MIDI_OPEN_FAILED;
    }
    SS_MIDIFile *midi = ss_midi_load(input_file, file_name ? file_name : "");
    ss_file_close(input_file);
    if (!midi) {
        return KOG_SPESSASYNTH_MIDI_PARSE_FAILED;
    }

    if (ss_midi_has_emidi(midi) && ss_midi_remove_emidi_non_gm(midi) > 0) {
        ss_midi_flush(midi);
    }

    SS_File *output_file = ss_file_open_blank_memory();
    if (!output_file) {
        ss_midi_free(midi);
        return KOG_SPESSASYNTH_MIDI_OPEN_FAILED;
    }
    if (!ss_midi_write(midi, output_file)) {
        ss_file_close(output_file);
        ss_midi_free(midi);
        return KOG_SPESSASYNTH_MIDI_WRITE_FAILED;
    }

    uint8_t *serialized = NULL;
    size_t serialized_size = 0;
    if (!ss_file_retrieve_memory(output_file, &serialized, &serialized_size)) {
        ss_file_close(output_file);
        ss_midi_free(midi);
        return KOG_SPESSASYNTH_MIDI_WRITE_FAILED;
    }
    if (serialized_size == 0 || serialized_size > KOG_SPESSASYNTH_MAX_OUTPUT) {
        ss_file_close(output_file);
        ss_midi_free(midi);
        return KOG_SPESSASYNTH_MIDI_OUTPUT_TOO_LARGE;
    }

    int result = copy_bytes(serialized, serialized_size, midi_data);
    if (result != KOG_SPESSASYNTH_MIDI_OK) {
        ss_file_close(output_file);
        ss_midi_free(midi);
        return result;
    }
    *midi_size = serialized_size;

    if (midi->rmidi_info.name && midi->rmidi_info.name_len > 0 &&
        midi->rmidi_info.name_len <= 4096) {
        result = copy_bytes(midi->rmidi_info.name, midi->rmidi_info.name_len,
                            title_data);
        if (result != KOG_SPESSASYNTH_MIDI_OK) {
            free(*midi_data);
            *midi_data = NULL;
            *midi_size = 0;
            ss_file_close(output_file);
            ss_midi_free(midi);
            return result;
        }
        *title_size = midi->rmidi_info.name_len;
    }

    ss_file_close(output_file);
    ss_midi_free(midi);
    return KOG_SPESSASYNTH_MIDI_OK;
}

void kog_spessasynth_midi_free(void *data) {
    free(data);
}
