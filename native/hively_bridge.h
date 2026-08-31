#ifndef KOG_HIVELY_BRIDGE_H
#define KOG_HIVELY_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogHively KogHively;

enum KogHivelyError {
    KOG_HIVELY_OK = 0,
    KOG_HIVELY_INVALID_FILE = 1,
    KOG_HIVELY_INVALID_SUBSONG = 2,
    KOG_HIVELY_OUT_OF_MEMORY = 3,
    KOG_HIVELY_DURATION_LIMIT = 4,
};

void kog_hively_init(void);
KogHively *kog_hively_open(
    const uint8_t *data,
    size_t data_size,
    uint32_t sample_rate,
    uint32_t subsong,
    uint32_t loop_count,
    uint64_t fade_frames,
    int *error
);
void kog_hively_free(KogHively *decoder);

uint32_t kog_hively_subsong_count(const KogHively *decoder);
uint32_t kog_hively_selected_subsong(const KogHively *decoder);
const char *kog_hively_title(const KogHively *decoder);
uint64_t kog_hively_total_frames(const KogHively *decoder);

size_t kog_hively_render(KogHively *decoder, float *output, size_t frames);
uint64_t kog_hively_seek(KogHively *decoder, uint64_t frame);

#ifdef __cplusplus
}
#endif

#endif
