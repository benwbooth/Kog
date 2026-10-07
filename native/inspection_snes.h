#ifndef KOG_INSPECTION_SNES_H
#define KOG_INSPECTION_SNES_H
#include "inspection.h"

// A BRR sample has no root-note metadata. Keep its pitch as a playback rate;
// noise also has no single musical key. ENVX represents the live envelope.
inline void kog_inspect_snes(const uint8_t* regs, KogVoices& voices) {
    for(unsigned i = 0; i < 8; ++i) {
        const auto* r = regs + 16 * i;
        const auto left = static_cast<int8_t>(r[0]);
        const auto right = static_cast<int8_t>(r[1]);
        const float level = (r[8] / 127.0f) * std::max(std::abs(int(left)), std::abs(int(right))) / 128.0f;
        const int pitch = r[2] | (r[3] & 63) << 8;
        char name[64]; std::snprintf(name, sizeof(name), "SNES DSP %u", i + 1);
        auto& v = voices.add(name, (regs[0x3d] & (1 << i)) ? 1 : 2,
            r[8] != 0 && !(regs[0x6c] & 0xc0), 0, level,
            (std::abs(int(right)) - std::abs(int(left))) / 128.0f);
        std::snprintf(v.instrument, sizeof(v.instrument), "BRR %02X", r[4]);
        std::snprintf(v.details, sizeof(v.details),
            "Pitch rate=%.1f Hz (%04X) | ADSR=%02X %02X | Gain=%02X | Envelope=%02X | Output=%02X | Volume L/R=%d/%d | Echo=%u | Pitch modulation=%u | Key on/off=%02X/%02X | DSP flags=%02X | Echo feedback=%d | Echo delay=%u | Sample directory=%02X",
            pitch * (32000.0 / 4096), pitch, r[5], r[6], r[7], r[8], r[9], left, right,
            unsigned((regs[0x4d] >> i) & 1), unsigned((regs[0x2d] >> i) & 1), regs[0x4c], regs[0x5c],
            regs[0x6c], static_cast<int8_t>(regs[0x0d]), regs[0x7d] & 15, regs[0x5d]);
    }
}
#endif
