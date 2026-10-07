// Compiled into the pinned C API through the build-directory overlay.
// Read the actual mixer voices, including background NNA voices and effects.
#include "common/stdafx.h"
#include "soundlib/Sndfile.h"
#include <cmath>

extern "C" size_t kog_openmpt_voices(openmpt_module *mod, float *out, size_t capacity) {
    if (!mod || !out) return 0;
    auto &song = mod->impl->kog_sound_file();
    size_t count = 0;
    for (size_t voice = 0; voice < song.m_PlayState.Chn.size() && count < capacity; ++voice) {
        const auto &ch = song.m_PlayState.Chn[voice];
        if (!ch.pCurrentSample || !ch.nLength || !ch.increment.GetRaw()) continue;
        const auto channel = voice < song.GetNumChannels() ? voice : (ch.nMasterChn ? ch.nMasterChn - 1 : voice);
        // C5Speed anchors the sample to the module's C-5 (MIDI C5 = 72).
        // The actual increment includes arpeggios, vibrato and pitch envelopes.
        const double rate = std::abs(ch.increment.ToDouble()) * song.GetSampleRate();
        const double key = ch.nC5Speed > 0 && rate > 0 ? 72.0 + 12.0 * std::log2(rate / ch.nC5Speed) : -1.0;
        size_t sample = 0;
        for (size_t i = 1; i <= song.GetNumSamples(); ++i) {
            if (&song.GetSample(static_cast<OpenMPT::SAMPLEINDEX>(i)) == ch.pModSample) { sample = i; break; }
        }
        auto *data = out + count++ * 8;
        data[0] = static_cast<float>(channel);
        data[1] = static_cast<float>(key);
        data[2] = std::min(1.0f, static_cast<float>(ch.nRealVolume) / 16384.0f);
        data[3] = (static_cast<float>(ch.nRealPan) - 128.0f) / 128.0f;
        data[4] = static_cast<float>(sample);
        data[5] = static_cast<float>(ch.nOldIns);
        data[6] = ch.dwFlags[OpenMPT::CHN_KEYOFF] ? 0.0f : 1.0f;
        data[7] = static_cast<float>(ch.nPeriod);
    }
    return count;
}
