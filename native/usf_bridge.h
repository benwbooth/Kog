// Kog C ABI wrapper for LazyUSF2 USF playback.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef KOG_USF_BRIDGE_H
#define KOG_USF_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogUsf KogUsf;

KogUsf *kog_usf_open(const char *path,
                     uint32_t default_length_milliseconds,
                     uint32_t default_fade_milliseconds);
void kog_usf_free(KogUsf *decoder);

uint32_t kog_usf_sample_rate(const KogUsf *decoder);
uint32_t kog_usf_channels(const KogUsf *decoder);
uint64_t kog_usf_total_frames(const KogUsf *decoder);
const char *kog_usf_title(const KogUsf *decoder);
const char *kog_usf_artist(const KogUsf *decoder);
const char *kog_usf_album(const KogUsf *decoder);
const char *kog_usf_genre(const KogUsf *decoder);
const char *kog_usf_date(const KogUsf *decoder);

int64_t kog_usf_render(KogUsf *decoder, float *output, size_t frames);
int64_t kog_usf_seek(KogUsf *decoder, uint64_t frame);
const char *kog_usf_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
