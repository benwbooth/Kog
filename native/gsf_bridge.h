#ifndef KOG_GSF_BRIDGE_H
#define KOG_GSF_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogGsf KogGsf;

KogGsf *kog_gsf_open(const char *path,
                     uint32_t default_length_milliseconds,
                     uint32_t default_fade_milliseconds);
void kog_gsf_free(KogGsf *decoder);

uint32_t kog_gsf_sample_rate(const KogGsf *decoder);
uint32_t kog_gsf_channels(const KogGsf *decoder);
uint64_t kog_gsf_total_frames(const KogGsf *decoder);
const char *kog_gsf_title(const KogGsf *decoder);
const char *kog_gsf_artist(const KogGsf *decoder);
const char *kog_gsf_album(const KogGsf *decoder);
const char *kog_gsf_genre(const KogGsf *decoder);
const char *kog_gsf_date(const KogGsf *decoder);

int64_t kog_gsf_render(KogGsf *decoder, float *output, size_t frames);
int64_t kog_gsf_seek(KogGsf *decoder, uint64_t frame);
const char *kog_gsf_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
