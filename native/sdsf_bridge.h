// Kog C ABI wrapper for Highly Theoretical SSF/DSF playback.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef KOG_SDSF_BRIDGE_H
#define KOG_SDSF_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogSdsf KogSdsf;

KogSdsf *kog_sdsf_open(const char *path,
                       uint32_t default_length_milliseconds,
                       uint32_t default_fade_milliseconds);
void kog_sdsf_free(KogSdsf *decoder);

uint32_t kog_sdsf_sample_rate(const KogSdsf *decoder);
uint32_t kog_sdsf_channels(const KogSdsf *decoder);
uint64_t kog_sdsf_total_frames(const KogSdsf *decoder);
uint8_t kog_sdsf_version(const KogSdsf *decoder);
const char *kog_sdsf_title(const KogSdsf *decoder);
const char *kog_sdsf_artist(const KogSdsf *decoder);
const char *kog_sdsf_album(const KogSdsf *decoder);
const char *kog_sdsf_genre(const KogSdsf *decoder);
const char *kog_sdsf_date(const KogSdsf *decoder);

int64_t kog_sdsf_render(KogSdsf *decoder, float *output, size_t frames);
int64_t kog_sdsf_seek(KogSdsf *decoder, uint64_t frame);
const char *kog_sdsf_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
