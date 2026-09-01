#ifndef KOG_MT32EMU_BRIDGE_H
#define KOG_MT32EMU_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct KogMt32 KogMt32;

KogMt32 *kog_mt32_open(const char *rom_directory,
                       uint32_t sample_rate,
                       char *error,
                       size_t error_size);
void kog_mt32_free(KogMt32 *synth);
const char *kog_mt32_model(const KogMt32 *synth);
uint32_t kog_mt32_sample_rate(const KogMt32 *synth);
int kog_mt32_send(KogMt32 *synth,
                  const uint8_t *bytes,
                  size_t length,
                  char *error,
                  size_t error_size);
int kog_mt32_render(KogMt32 *synth,
                    float *output,
                    size_t frames,
                    char *error,
                    size_t error_size);

#ifdef __cplusplus
}
#endif

#endif
