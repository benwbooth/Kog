// Kog C ABI wrapper for Highly Quixotic QSF playback.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef KOG_QSF_BRIDGE_H
#define KOG_QSF_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogQsf KogQsf;

KogQsf *kog_qsf_open(const char *path,
                     uint32_t default_length_milliseconds,
                     uint32_t default_fade_milliseconds);
void kog_qsf_free(KogQsf *decoder);

uint32_t kog_qsf_sample_rate(const KogQsf *decoder);
uint32_t kog_qsf_channels(const KogQsf *decoder);
uint64_t kog_qsf_total_frames(const KogQsf *decoder);
const char *kog_qsf_title(const KogQsf *decoder);
const char *kog_qsf_artist(const KogQsf *decoder);
const char *kog_qsf_album(const KogQsf *decoder);
const char *kog_qsf_genre(const KogQsf *decoder);
const char *kog_qsf_date(const KogQsf *decoder);

int64_t kog_qsf_render(KogQsf *decoder, float *output, size_t frames);
int64_t kog_qsf_seek(KogQsf *decoder, uint64_t frame);
const char *kog_qsf_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
