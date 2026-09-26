// SPDX-License-Identifier: GPL-3.0-or-later
#include "dsp.hpp"
#include "../smp/smp.hpp"
#include <algorithm>
namespace SuperFamicom {
DSP::DSP(SMP& source) : owner(source) {}
void DSP::enter() {
  // Keep a partial final frame across render calls, and never overrun the
  // DSP output array even when restoring a capture's negative clock phase.
  const auto cycles = std::min<int64_t>(256, -clock / (24 * 4096) + 1);
  spc_dsp.run(static_cast<int>(cycles));
  clock += cycles * 24 * 4096;
  const auto available = static_cast<unsigned>(spc_dsp.sample_count());
  while(consumed < available) {
    if(!owner.sample(buffer[consumed], buffer[consumed + 1])) return;
    consumed += 2;
  }
  if(available) {
    spc_dsp.set_output(buffer, 8192);
    consumed = 0;
  }
}
void DSP::power() {
  spc_dsp.init(owner.apuram);
  spc_dsp.reset();
  spc_dsp.set_output(buffer, 8192);
  consumed = 0;
  clock = 0;
}
void DSP::reset() {
  spc_dsp.soft_reset();
  spc_dsp.set_output(buffer, 8192);
  consumed = 0;
  clock = 0;
}
bool DSP::mute() { return spc_dsp.mute(); }
uint8_t DSP::read(uint8_t address) { return spc_dsp.read(address); }
void DSP::write(uint8_t address, uint8_t value) { spc_dsp.write(address, value); }
void DSP::channel_enable(unsigned channel, bool enabled) {
  const unsigned bit = 1u << (channel & 7);
  mute_mask = enabled ? mute_mask & ~bit : mute_mask | bit;
  spc_dsp.mute_voices(mute_mask);
}
void DSP::disable_surround(bool disabled) { spc_dsp.disable_surround(disabled); }
}
