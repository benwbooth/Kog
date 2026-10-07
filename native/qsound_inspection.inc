/* Included after the pinned QSound core, so every field uses its real layout. */
#include "inspection.h"
#include <stdio.h>
#include <math.h>
size_t kog_qsoundc_inspect(void* info, KogVoice* out, size_t capacity) {
    const struct qsound_chip* chip = (const struct qsound_chip*)info;
    const size_t count = capacity < 19 ? capacity : 19;
    size_t i;
    if(!chip || !out) return 0;
    for(i = 0; i < count; ++i) {
        KogVoice* v = out + i;
        memset(v, 0, sizeof(*v));
        v->id = (uint32_t)i; v->kind = 2; v->key = -1;
        v->pan = ((int)chip->voice_pan[i] - 0x120) / 16.0f;
        if(i < 16) {
            const struct qsound_voice* c = chip->voice + i;
            v->active = c->rate != 0 && c->volume != 0;
            v->level = fabsf(c->volume / 32768.0f);
            snprintf(v->name, sizeof(v->name), "QSound PCM %u", (unsigned)i + 1);
            snprintf(v->instrument, sizeof(v->instrument), "Bank %04X", c->bank);
            snprintf(v->details, sizeof(v->details),
                "Pitch rate=%.1f Hz (%04X) | Address=%04X | End=%04X | Loop length=%d | Volume=%d | Pan=%04X | Echo=%d | DSP state=%04X | Echo feedback=%d",
                24038.0 * c->rate / 4096.0, c->rate, (uint16_t)c->addr, (uint16_t)c->end_addr,
                c->loop_len, c->volume, chip->voice_pan[i], c->echo, chip->state, chip->echo.feedback);
        } else {
            const struct qsound_adpcm* c = chip->adpcm + i - 16;
            v->active = c->cur_vol != 0;
            v->level = fabsf(c->cur_vol / 32768.0f);
            snprintf(v->name, sizeof(v->name), "QSound ADPCM %u", (unsigned)i - 15);
            snprintf(v->instrument, sizeof(v->instrument), "Bank %04X", c->bank);
            snprintf(v->details, sizeof(v->details),
                "Start=%04X | End=%04X | Address=%04X | Volume=%d | Current volume=%d | Flags=%04X | Step=%d | Pan=%04X",
                c->start_addr, c->end_addr, c->cur_addr, c->volume, c->cur_vol, c->flag, c->step_size, chip->voice_pan[i]);
        }
    }
    return count;
}
