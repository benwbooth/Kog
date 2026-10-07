/* N64 has a stereo DMA interface; software voices depend on the microcode. */
#include "inspection.h"
#include "usf/usf.h"
#include "usf/usf_internal.h"
#include <stdio.h>
#include <string.h>

size_t kog_usf_core_inspect(void* state, KogVoice* out, size_t capacity) {
    size_t count = capacity < 2 ? capacity : 2, i;
    const usf_state_t* core;
    if(!state || !out) return 0;
    core = USF_STATE;
    for(i = 0; i < count; ++i) {
        KogVoice* v = out + i;
        memset(v, 0, sizeof(*v));
        v->id = (uint32_t)i; v->kind = 4; v->key = -1;
        v->pan = i ? 1 : -1;
        snprintf(v->name, sizeof(v->name), "N64 AI %s", i ? "Right" : "Left");
        snprintf(v->instrument, sizeof(v->instrument), "Software-mixed PCM");
        snprintf(v->details, sizeof(v->details),
            "Sample rate=%d Hz | DMA address=%08X | DMA length=%u | Status=%08X | Control=%08X | DAC rate=%u | Bit rate=%u | HLE audio=%u",
            core->SampleRate, core->g_ai.regs[AI_DRAM_ADDR_REG], core->g_ai.regs[AI_LEN_REG],
            core->g_ai.regs[AI_STATUS_REG], core->g_ai.regs[AI_CONTROL_REG], core->g_ai.regs[AI_DACRATE_REG],
            core->g_ai.regs[AI_BITRATE_REG], core->enable_hle_audio);
    }
    return count;
}
