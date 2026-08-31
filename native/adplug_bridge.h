#ifndef KOG_ADPLUG_BRIDGE_H
#define KOG_ADPLUG_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogAdPlug KogAdPlug;

enum KogAdPlugError {
    KOG_ADPLUG_OK = 0,
    KOG_ADPLUG_INVALID_ARGUMENT = 1,
    KOG_ADPLUG_OPEN_FAILED = 2,
    KOG_ADPLUG_INVALID_SUBSONG = 3,
    KOG_ADPLUG_OUT_OF_MEMORY = 4,
};

KogAdPlug *kog_adplug_open(
    const char *path,
    uint32_t subsong,
    uint32_t sample_rate,
    int *error
);
void kog_adplug_free(KogAdPlug *decoder);

uint32_t kog_adplug_sample_rate(const KogAdPlug *decoder);
uint32_t kog_adplug_subsong_count(const KogAdPlug *decoder);
uint64_t kog_adplug_total_frames(const KogAdPlug *decoder);
const char *kog_adplug_type(const KogAdPlug *decoder);
const char *kog_adplug_title(const KogAdPlug *decoder);
const char *kog_adplug_author(const KogAdPlug *decoder);

int64_t kog_adplug_render(KogAdPlug *decoder, float *output, size_t frames);
int64_t kog_adplug_seek(KogAdPlug *decoder, uint64_t frame);

int kog_adplug_supports_extension(const char *extension);
size_t kog_adplug_extension_count(void);
const char *kog_adplug_extension(size_t index);
const char *kog_adplug_version(void);

#ifdef __cplusplus
}
#endif

#endif
