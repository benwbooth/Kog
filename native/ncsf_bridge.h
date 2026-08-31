#ifndef KOG_NCSF_BRIDGE_H
#define KOG_NCSF_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogNcsf KogNcsf;

KogNcsf *kog_ncsf_open(const char *path,
                       uint32_t default_length_milliseconds,
                       uint32_t default_fade_milliseconds);
void kog_ncsf_free(KogNcsf *decoder);

uint32_t kog_ncsf_sample_rate(const KogNcsf *decoder);
uint32_t kog_ncsf_channels(const KogNcsf *decoder);
uint64_t kog_ncsf_total_frames(const KogNcsf *decoder);
const char *kog_ncsf_title(const KogNcsf *decoder);
const char *kog_ncsf_artist(const KogNcsf *decoder);
const char *kog_ncsf_album(const KogNcsf *decoder);
const char *kog_ncsf_genre(const KogNcsf *decoder);
const char *kog_ncsf_date(const KogNcsf *decoder);

int64_t kog_ncsf_render(KogNcsf *decoder, float *output, size_t frames);
int64_t kog_ncsf_seek(KogNcsf *decoder, uint64_t frame);
const char *kog_ncsf_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
