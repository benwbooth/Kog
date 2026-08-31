#ifndef KOG_SID_BRIDGE_H
#define KOG_SID_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogSid KogSid;

KogSid *kog_sid_open(const uint8_t *data,
                     size_t data_size,
                     uint32_t subsong,
                     uint32_t sample_rate,
                     uint32_t play_seconds,
                     uint32_t fade_milliseconds);
void kog_sid_free(KogSid *decoder);

uint32_t kog_sid_sample_rate(const KogSid *decoder);
uint32_t kog_sid_channels(const KogSid *decoder);
uint32_t kog_sid_subsong_count(const KogSid *decoder);
uint32_t kog_sid_selected_subsong(const KogSid *decoder);
uint64_t kog_sid_total_frames(const KogSid *decoder);
const char *kog_sid_title(const KogSid *decoder);
const char *kog_sid_artist(const KogSid *decoder);
const char *kog_sid_released(const KogSid *decoder);
const char *kog_sid_format(const KogSid *decoder);

int64_t kog_sid_render(KogSid *decoder, float *output, size_t frames);
int64_t kog_sid_seek(KogSid *decoder, uint64_t frame);
const char *kog_sid_last_error(void);
const char *kog_sid_version(void);

#ifdef __cplusplus
}
#endif

#endif
