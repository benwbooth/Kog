#ifndef KOG_INSPECTION_OPL_H
#define KOG_INSPECTION_OPL_H
#include "inspection.h"

// OPL key gates and programmed parameters; levels are carrier attenuation,
// rather than a claim to reconstruct the chip's envelope output.
inline void kog_inspect_opl(const uint8_t *regs, KogVoices &voices, const char *prefix, double clock = 14318180.0) {
    const bool opl3 = regs[0x105] & 1;
    const bool rhythm = regs[0xbd] & 0x20;
    const int slots[] = {0, 1, 2, 8, 9, 10, 16, 17, 18};
    for (int i = 0; i < (opl3 ? 18 : 9); ++i) {
        const int bank = i / 9 * 256;
        const int c = i % 9;
        if (rhythm && bank == 0 && c >= 6) continue;
        const auto *r = regs + bank;
        const int slot = slots[c];
        const int fnum = r[0xa0 + c] | (r[0xb0 + c] & 3) << 8;
        const int block = (r[0xb0 + c] >> 2) & 7;
        const bool secondary = c >= 3 && c < 6 && opl3 && (regs[0x104] & (1 << (c - 3 + i / 9 * 3)));
        const bool four_op = c < 3 && opl3 && (regs[0x104] & (1 << (c + i / 9 * 3)));
        const int carrier = four_op ? slots[c + 3] + 3 : slot + 3;
        const int tl = r[0x40 + carrier] & 63;
        const float volume = std::pow(10.0f, -0.75f * tl / 20.0f);
        char name[64]; std::snprintf(name, sizeof(name), "%s FM %d%s", prefix, i + 1, four_op ? " (4-op)" : secondary ? " (paired)" : "");
        auto &v = voices.add(name, 0, !secondary && (r[0xb0 + c] & 0x20), clock / 288.0 * fnum * (1 << block) / 1048576.0, volume,
            opl3 ? ((r[0xc0 + c] & 0x20 ? 1.0f : 0.0f) - (r[0xc0 + c] & 0x10 ? 1.0f : 0.0f)) : 0.0f);
        v.id = static_cast<uint32_t>(i);
        std::snprintf(v.instrument, sizeof(v.instrument), "%d-op · Wave %X/%X", four_op ? 4 : 2, r[0xe0 + slot] & 7, r[0xe3 + slot] & 7);
        std::snprintf(v.details, sizeof(v.details), "F-number=%03X | Block=%d | Key/pitch=%02X | Algorithm/feedback=%02X | Modulator=%02X %02X %02X %02X %02X | Carrier=%02X %02X %02X %02X %02X | Rhythm/LFO=%02X",
            fnum, block, r[0xb0 + c], r[0xc0 + c], r[0x20 + slot], r[0x40 + slot], r[0x60 + slot], r[0x80 + slot], r[0xe0 + slot],
            r[0x20 + carrier], r[0x40 + carrier], r[0x60 + carrier], r[0x80 + carrier], r[0xe0 + carrier], regs[0xbd]);
    }
    if (rhythm) {
        const char *names[] = {"Bass drum", "Snare", "Tom", "Cymbal", "Hi-hat"};
        const int bits[] = {16, 8, 4, 2, 1};
        const int operators[] = {19, 20, 18, 21, 17};
        for (int i = 0; i < 5; ++i) {
            const int slot = operators[i];
            char name[64]; std::snprintf(name, sizeof(name), "%s %s", prefix, names[i]);
            auto &v = voices.add(name, 3, regs[0xbd] & bits[i], 0, std::pow(10.0f, -0.75f * (regs[0x40 + slot] & 63) / 20.0f));
            v.id = 18 + i;
            std::snprintf(v.details, sizeof(v.details), "Rhythm=%02X | Operator=%02X %02X %02X %02X %02X", regs[0xbd], regs[0x20 + slot], regs[0x40 + slot], regs[0x60 + slot], regs[0x80 + slot], regs[0xe0 + slot]);
        }
    }
}
#endif
