// Kog C ABI wrapper for Cog's OPL3Windows MIDI synthesizer.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#include "kog_opl3w.h"

#include "interface.h"

#include <new>

struct kog_opl3w {
	struct midisynth *synth;
};

extern "C" kog_opl3w *kog_opl3w_create(uint32_t sample_rate) {
	try {
		midisynth *synth = getsynth_opl3w();
		if(!synth) return nullptr;
		if(!synth->midi_init(sample_rate, 0, 0)) {
			delete synth;
			return nullptr;
		}
		kog_opl3w *wrapper = new(std::nothrow) kog_opl3w{synth};
		if(!wrapper) delete synth;
		return wrapper;
	} catch(...) {
		return nullptr;
	}
}

extern "C" void kog_opl3w_destroy(kog_opl3w *synth) {
	if(!synth) return;
	delete synth->synth;
	delete synth;
}

extern "C" void kog_opl3w_write(kog_opl3w *synth, uint32_t packed_midi) {
	if(synth && synth->synth) synth->synth->midi_write(packed_midi);
}

extern "C" void kog_opl3w_generate(kog_opl3w *synth, int16_t *stereo,
	                                  uint32_t frames) {
	if(synth && synth->synth && stereo && frames)
		synth->synth->midi_generate(stereo, frames);
}
