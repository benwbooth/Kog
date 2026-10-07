#include "inspection.h"
#include "Music_Emu.h"
#include "Multi_Buffer.h"
#include "Nsf_Emu.h"
#include "Nes_Vrc6_Apu.h"
#include "Nes_Namco_Apu.h"
#include "Nes_Fme7_Apu.h"
#include "Nes_Fds_Apu.h"
#include "Nes_Mmc5_Apu.h"
#include "Nes_Vrc7_Apu.h"
#include "Gbs_Emu.h"
#include "Spc_Emu.h"
#include "Ay_Emu.h"
#include "Hes_Emu.h"
#include "Kss_Emu.h"
#include "Sap_Emu.h"

// The build overlay grants this reader friendship; all state belongs to the
// emulator generating the audio. No CPU or sound chip is run a second time.
struct KogGmeInspector {
    KogVoices voices;
    double ahead = 0.0;

    void classic(Classic_Emu &emu) {
        ahead += double(emu.buf->samples_avail()) / (emu.out_channels() * emu.sample_rate());
    }
    void square(Nes_Square &osc, double clock, const char *name) {
        const int period = osc.period();
        const bool active = osc.length_counter > 0 && osc.volume() > 0 && period >= 8;
        auto &v = voices.add(name, 0, active, clock / (16.0 * (period + 1)), osc.volume() / 15.0f);
        std::snprintf(v.instrument, sizeof(v.instrument), "Pulse duty %d", osc.regs[0] >> 6);
        std::snprintf(v.details, sizeof(v.details), "Period=%03X | Length=%d | Envelope=%d | Sweep=%02X", period, osc.length_counter, osc.envelope, osc.regs[1]);
    }
    void ay_regs(const unsigned char *regs, double clock, const char *prefix, const Ay_Apu *apu = nullptr) {
        for (int i = 0; i < 3; ++i) {
            const int period = (regs[i * 2 + 1] & 15) * 256 + regs[i * 2];
            const bool tone = !(regs[7] & (1 << i));
            const bool noise = !(regs[7] & (8 << i));
            const bool envelope = regs[8 + i] & 16;
            const float level = envelope && apu ? apu->env.wave[apu->env.pos] / 255.0f : (regs[8 + i] & 15) / 15.0f;
            char name[64]; std::snprintf(name, sizeof(name), "%s %d", prefix, i + 1);
            auto &v = voices.add(name, noise ? (tone ? 4 : 1) : 0, (tone || noise) && level > 0, tone ? clock / (32.0 * std::max(1, period)) : 0.0, level);
            std::snprintf(v.instrument, sizeof(v.instrument), "%s%s%s", tone ? "Tone" : "", tone && noise ? " + " : "", noise ? "Noise" : "");
            std::snprintf(v.details, sizeof(v.details), "Period=%03X | Noise=%02X | Mixer=%02X | Volume=%02X | Envelope=%04X:%02X", period, regs[6], regs[7], regs[8 + i], regs[11] | regs[12] << 8, regs[13]);
        }
    }
    void nsf(Nsf_Emu &emu) {
        classic(emu);
        auto &apu = emu.apu;
        const double clock = emu.clock_rate_;
        square(apu.square1, clock, "2A03 Pulse 1");
        square(apu.square2, clock, "2A03 Pulse 2");
        auto &tri = apu.triangle;
        auto &v = voices.add("2A03 Triangle", 0, tri.length_counter > 0 && tri.linear_counter > 0 && tri.period() > 1, clock / (32.0 * (tri.period() + 1)), 1.0f);
        std::snprintf(v.details, sizeof(v.details), "Period=%03X | Length=%d | Linear=%d", tri.period(), tri.length_counter, tri.linear_counter);
        auto &noise = voices.add("2A03 Noise", 1, apu.noise.length_counter > 0 && apu.noise.volume() > 0, 0, apu.noise.volume() / 15.0f);
        std::snprintf(noise.details, sizeof(noise.details), "Rate/mode=%02X | Envelope=%d | Length=%d", apu.noise.regs[2], apu.noise.envelope, apu.noise.length_counter);
        auto &pcm = voices.add("2A03 DMC", 2, !apu.dmc.silence, 0, apu.dmc.dac / 127.0f);
        std::snprintf(pcm.details, sizeof(pcm.details), "DAC=%02X | Address=%04X | Bytes=%d | Period=%d | Loop=%d", apu.dmc.dac, apu.dmc.address, apu.dmc.length_counter, apu.dmc.period, !!(apu.dmc.regs[0] & 0x40));
        if (emu.vrc6) for (int i : {2, 0, 1}) {
            auto &osc = emu.vrc6->oscs[i];
            const int volume = osc.regs[0] & (i == 2 ? 63 : 15);
            auto &voice = voices.add(i == 2 ? "VRC6 Saw" : i == 0 ? "VRC6 Pulse 1" : "VRC6 Pulse 2", 0,
                (osc.regs[2] & 0x80) && volume > 0, clock / ((i == 2 ? 14.0 : 16.0) * osc.period()), volume / float(i == 2 ? 63 : 15));
            std::snprintf(voice.details, sizeof(voice.details), "Period=%03X | Control=%02X | Enable=%02X", osc.period() - 1, osc.regs[0], osc.regs[2]);
        }
        if (emu.namco) {
            const auto *regs = emu.namco->reg;
            const int active = (regs[0x7f] >> 4 & 7) + 1;
            for (int i = 0; i < 8; ++i) {
                const auto *r = regs + 0x40 + i * 8;
                const int freq = ((r[4] & 3) << 16) | (r[2] << 8) | r[0];
                const int length = 32 - (r[4] >> 2 & 7) * 4;
                char name[64]; std::snprintf(name, sizeof(name), "N163 Wave %d", i + 1);
                auto &voice = voices.add(name, 0, i >= 8 - active && (r[7] & 15) > 0 && (r[4] & 0xe0),
                    clock * freq / (983040.0 * active * length), (r[7] & 15) / 15.0f);
                std::snprintf(voice.instrument, sizeof(voice.instrument), "Wave %02X", r[6]);
                std::snprintf(voice.details, sizeof(voice.details), "Frequency=%05X | Wave length=%d | Volume=%X | Active voices=%d", freq, length, r[7] & 15, active);
            }
        }
        if (emu.fme7) ay_regs(emu.fme7->regs, clock, "FME7");
        if (emu.fds) {
            auto &fds = *emu.fds;
            int freq = (fds.regs(0x4083) & 15) * 256 + fds.regs(0x4082);
            if (!(fds.regs(0x4087) & 0x80) && ((fds.regs(0x4087) & 15) || fds.regs(0x4086))) {
                const int bias = (fds.regs(0x4085) ^ 0x40) - 0x40;
                int factor = bias * fds.sweep_gain;
                const int extra = factor & 15;
                factor >>= 4;
                if (extra) { --factor; if (bias >= 0) factor += 3; }
                if (factor > 193) factor -= 258;
                if (factor < -64) factor += 256;
                freq += (freq * factor) >> 6;
            }
            auto &voice = voices.add("FDS Wave", 0, !(fds.regs(0x4083) & 0x80) && fds.env_gain > 0,
                clock * std::max(0, freq) / 4194304.0, std::min(32, fds.env_gain) / 32.0f);
            std::snprintf(voice.details, sizeof(voice.details), "Frequency=%03X | Envelope=%d | Sweep=%d | Modulation=%02X:%02X | Wave control=%02X", freq, fds.env_gain, fds.sweep_gain, fds.regs(0x4086), fds.regs(0x4087), fds.regs(0x4089));
        }
        if (emu.mmc5) {
            square(emu.mmc5->square1, clock, "MMC5 Pulse 1");
            square(emu.mmc5->square2, clock, "MMC5 Pulse 2");
            auto &voice = voices.add("MMC5 DAC", 2, emu.mmc5->dmc.dac != 0, 0, emu.mmc5->dmc.dac / 127.0f);
            std::snprintf(voice.details, sizeof(voice.details), "DAC=%02X", emu.mmc5->dmc.dac);
        }
        if (emu.vrc7) for (int i = 0; i < 6; ++i) {
            const auto *r = emu.vrc7->oscs[i].regs;
            const int fnum = r[0] | (r[1] & 1) << 8;
            const int block = r[1] >> 1 & 7;
            char name[64]; std::snprintf(name, sizeof(name), "VRC7 FM %d", i + 1);
            auto &voice = voices.add(name, 0, r[1] & 0x10, 49716.0 * fnum * (1 << block) / 524288.0, (15 - (r[2] & 15)) / 15.0f);
            std::snprintf(voice.instrument, sizeof(voice.instrument), "Patch %X", r[2] >> 4);
            std::snprintf(voice.details, sizeof(voice.details), "F-number=%03X | Block=%d | Control=%02X | Attenuation=%X", fnum, block, r[1], r[2] & 15);
        }
    }
    void gbs(Gbs_Emu &emu) {
        classic(emu);
        for (int i = 0; i < 4; ++i) {
            const auto &osc = *emu.apu.oscs[i];
            const int frequency = osc.frequency();
            const bool wave = i == 2;
            const bool noise = i == 3;
            float level = osc.volume / 15.0f;
            if (wave) { const int shift = osc.regs[2] >> 5 & 3; level = shift ? 1.0f / (1 << (shift - 1)) : 0.0f; }
            const char *names[] = {"GB Pulse 1", "GB Pulse 2", "GB Wave", "GB Noise"};
            auto &v = voices.add(names[i], noise ? 1 : 0, osc.enabled && level > 0,
                noise ? 0.0 : (wave ? 65536.0 : 131072.0) / std::max(1, 2048 - frequency), level,
                osc.output_select == 1 ? 1.0f : osc.output_select == 2 ? -1.0f : 0.0f);
            std::snprintf(v.details, sizeof(v.details), "Frequency=%03X | Length=%d | Envelope=%02X | Control=%02X | Routing=%d", frequency, osc.length, osc.regs[2], osc.regs[4], osc.output_select);
        }
    }
    void spc(Spc_Emu &emu) {
        ahead += double(emu.resampler.avail()) / (2 * emu.sample_rate());
        const auto &dsp = emu.apu.dsp;
        const auto *regs = dsp.m.regs;
        for (int i = 0; i < 8; ++i) {
            const auto *r = regs + i * 16;
            const auto &osc = dsp.m.voices[i];
            const bool noise = regs[0x3d] & (1 << i);
            const int pitch = r[2] | (r[3] & 63) << 8;
            const float left = std::abs(int(static_cast<int8_t>(r[0]))) / 128.0f;
            const float right = std::abs(int(static_cast<int8_t>(r[1]))) / 128.0f;
            char name[64]; std::snprintf(name, sizeof(name), "SPC Voice %d", i + 1);
            auto &v = voices.add(name, noise ? 1 : 2, osc.env > 0, 0,
                osc.env / 2047.0f * std::max(left, right), right - left);
            std::snprintf(v.instrument, sizeof(v.instrument), "BRR sample %02X", r[4]);
            std::snprintf(v.details, sizeof(v.details), "Pitch=%04X (%.0f Hz sample rate) | ADSR=%02X:%02X | Gain=%02X | Envelope=%03X | BRR=%04X | Echo=%d | Pitch modulation=%d",
                pitch, pitch * 32000.0 / 4096.0, r[5], r[6], r[7], osc.env, osc.brr_addr, !!(regs[0x4d] & (1 << i)), !!(regs[0x2d] & (1 << i)));
        }
    }
    void ay(Ay_Emu &emu) {
        classic(emu);
        ay_regs(emu.apu.regs, emu.clock_rate(), "AY", &emu.apu);
        auto &v = voices.add("Spectrum beeper", 2, emu.last_beeper != 0, 0, emu.last_beeper ? 1.0f : 0.0f);
        std::snprintf(v.details, sizeof(v.details), "DAC=%d | Machine=%s", emu.last_beeper, emu.cpc_mode ? "CPC" : "Spectrum");
    }
    void hes(Hes_Emu &emu) {
        classic(emu);
        for (int i = 0; i < 6; ++i) {
            const auto &osc = emu.apu.oscs[i];
            const bool noise = i >= 4 && (osc.noise & 0x80);
            const bool dac = osc.control & 0x40;
            char name[64]; std::snprintf(name, sizeof(name), "HuC6280 Voice %d", i + 1);
            auto &v = voices.add(name, dac ? 2 : noise ? 1 : 0, (osc.control & 0x80) && (osc.control & 31),
                !noise && !dac && osc.period > 0 ? 7159091.0 / (64.0 * osc.period) : 0.0, (osc.control & 31) / 31.0f,
                (int(osc.balance & 15) - int(osc.balance >> 4)) / 15.0f);
            std::snprintf(v.details, sizeof(v.details), "Period=%03X | Control=%02X | Balance=%02X | Noise=%02X | DAC=%02X", osc.period, osc.control, osc.balance, osc.noise, osc.dac);
        }
        const auto &ad = emu.adpcm.state;
        auto &v = voices.add("PCE ADPCM", 2, ad.playflag, 0, ad.volume / 255.0f);
        std::snprintf(v.details, sizeof(v.details), "Rate=%d | Address=%04X | Remaining=%d | Repeat=%d", ad.freq, ad.playptr, ad.playlength, ad.repeatflag);
    }
    void kss(Kss_Emu &emu) {
        classic(emu);
        if (emu.sn) {
            for (int i = 0; i < 3; ++i) {
                auto &osc = emu.sn->squares[i];
                char name[64]; std::snprintf(name, sizeof(name), "SN76489 Tone %d", i + 1);
                auto &v = voices.add(name, 0, osc.volume > 0 && osc.period > 128, osc.period > 0 ? 3579545.0 / (2 * osc.period) : 0, osc.volume / 64.0f);
                std::snprintf(v.details, sizeof(v.details), "Period=%d | Volume=%d | Routing=%d", osc.period, osc.volume, osc.output_select);
            }
            auto &v = voices.add("SN76489 Noise", 1, emu.sn->noise.volume > 0, 0, emu.sn->noise.volume / 64.0f);
            std::snprintf(v.details, sizeof(v.details), "Period=%d | Feedback=%X", *emu.sn->noise.period, emu.sn->noise.feedback);
        } else {
            ay_regs(emu.ay.regs, 3579545.0, "MSX PSG", &emu.ay);
            const auto *r = emu.scc.regs;
            for (int i = 0; i < 5; ++i) {
                const int period = r[0x80 + i * 2] | (r[0x81 + i * 2] & 15) << 8;
                char name[64]; std::snprintf(name, sizeof(name), "SCC Wave %d", i + 1);
                auto &v = voices.add(name, 0, (r[0x8f] & (1 << i)) && (r[0x8a + i] & 15), 3579545.0 / (32.0 * (period + 1)), (r[0x8a + i] & 15) / 15.0f);
                std::snprintf(v.details, sizeof(v.details), "Period=%03X | Volume=%X | Enable=%02X", period, r[0x8a + i], r[0x8f]);
            }
        }
    }
    void sap(Sap_Emu &emu) {
        classic(emu);
        for (int chip = 0; chip < (emu.info.stereo ? 2 : 1); ++chip) {
            auto &apu = chip ? emu.apu2 : emu.apu;
            // run_until has already cached each oscillator's timer divisor.
            for (int i = 0; i < 4; ++i) {
                const auto &osc = apu.oscs[i];
                const bool tone = (osc.regs[1] & 0xa0) == 0xa0;
                const bool dac = osc.regs[1] & 0x10;
                char name[64]; std::snprintf(name, sizeof(name), "POKEY %d · Voice %d", chip + 1, i + 1);
                auto &v = voices.add(name, dac ? 2 : tone ? 0 : 1, (osc.regs[1] & 15) > 0,
                    tone && !dac && osc.period > 0 ? 1773447.0 / (2 * osc.period) : 0.0, (osc.regs[1] & 15) / 15.0f,
                    emu.info.stereo ? (chip ? 1.0f : -1.0f) : 0.0f);
                std::snprintf(v.details, sizeof(v.details), "AUDF=%02X | AUDC=%02X | AUDCTL=%02X | Period=%d", osc.regs[0], osc.regs[1], apu.control, osc.period);
            }
        }
    }
    void inspect(Music_Emu &emu) {
        ahead = double(std::max(0, emu.emu_time - emu.out_time)) / (emu.out_channels() * emu.sample_rate());
        const auto type = emu.type();
        if (type == gme_nsf_type || type == gme_nsfe_type) nsf(static_cast<Nsf_Emu &>(emu));
        else if (type == gme_gbs_type) gbs(static_cast<Gbs_Emu &>(emu));
        else if (type == gme_spc_type) spc(static_cast<Spc_Emu &>(emu));
        else if (type == gme_ay_type) ay(static_cast<Ay_Emu &>(emu));
        else if (type == gme_hes_type) hes(static_cast<Hes_Emu &>(emu));
        else if (type == gme_kss_type) kss(static_cast<Kss_Emu &>(emu));
        else if (type == gme_sap_type) sap(static_cast<Sap_Emu &>(emu));
    }
};

extern "C" void kog_gme_inspection_enable(Music_Emu *emu, int enabled) {
    if (emu) emu->kog_inspecting = enabled != 0;
}
extern "C" size_t kog_gme_inspection(Music_Emu *emu, KogVoice *voices, size_t capacity, double *ahead) {
    if (!emu || !voices || !ahead) return 0;
    KogGmeInspector inspector{{voices, capacity}};
    inspector.inspect(*emu);
    *ahead = inspector.ahead;
    return inspector.voices.count;
}
