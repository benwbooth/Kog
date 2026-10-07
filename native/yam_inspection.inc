/* Included after Highly Theoretical's core; no duplicate state layout. */
#include "inspection.h"
#include <stdio.h>
#include <math.h>
size_t kog_yam_inspect(void* info, KogVoice* out, size_t capacity) {
    const struct YAM_STATE* state = (const struct YAM_STATE*)info;
    size_t count, i;
    if(!state || !out) return 0;
    count = state->version == 1 ? 32 : 64;
    if(count > capacity) count = capacity;
    for(i = 0; i < count; ++i) {
        const struct YAM_CHAN* c = state->chan + i;
        KogVoice* v = out + i;
        const double rate = 44100.0 * (c->fns ^ 0x400) * (1u << (c->oct ^ 8)) / 262144.0;
        const unsigned attenuation = (c->tl << 2) + (c->envlevel & c->envlevelmask[c->envstate & 3]);
        memset(v, 0, sizeof(*v));
        v->id = (uint32_t)i; v->kind = c->ssctl ? 1 : 2; v->key = -1;
        v->active = c->envlevel < 0x3c0;
        v->level = v->active ? (c->voff ? 1.0f : powf(2.0f, -(float)attenuation / 64.0f)) : 0;
        v->pan = (c->dipan & 15) / 15.0f * (c->dipan & 16 ? -1.0f : 1.0f);
        snprintf(v->name, sizeof(v->name), "%s %u", state->version == 1 ? "SCSP" : "AICA", (unsigned)i + 1);
        snprintf(v->instrument, sizeof(v->instrument), "%s %06X", c->ssctl ? "Noise" : c->pcms >= 2 ? "ADPCM" : c->pcms ? "PCM8" : "PCM16", c->sampleaddr);
        snprintf(v->details, sizeof(v->details),
            "Pitch rate=%.1f Hz | Octave/FNS=%X/%03X | Position=%u | Loop=%d..%d mode %u | Gate=%u | Envelope=%04X state %u | ADSR=%u/%u/%u/%u | Level=%u | Direct=%u pan %02X | DSP=%u send %u | Pitch LFO=%u/%u | Amplitude LFO=%u/%u | LFO rate=%u | Filter=%u/%u | Ring=%u/%u/%u",
            rate, c->oct, c->fns, c->playpos, c->loopstart, c->loopend, c->sampler_looptype,
            c->kyonb, c->envlevel, c->envstate, c->ar[0], c->ar[1], c->ar[2], c->ar[3], c->tl,
            c->disdl, c->dipan, c->dspchan, c->dsplevel, c->plfows, c->plfos, c->alfows, c->alfos,
            c->lfof, c->lpflevel, c->q, c->mdl, c->mdxsl, c->mdysl);
    }
    return count;
}
