// SPDX-License-Identifier: GPL-3.0-or-later
// Kog's per-stream bridge to the LGPL SPC_DSP snapshot implementation.
#pragma once
#include "SPC_DSP.h"
#include <cstdint>
namespace SuperFamicom {
struct SMP;
struct DSP {
  explicit DSP(SMP& owner);
  int64_t clock = 0;
  SPC_DSP spc_dsp;
  void enter();
  void power();
  void reset();
  bool mute();
  uint8_t read(uint8_t address);
  void write(uint8_t address, uint8_t value);
  void channel_enable(unsigned channel, bool enabled);
  void disable_surround(bool disabled = true);
private:
  SMP& owner;
  int16_t buffer[8192] {};
  unsigned consumed = 0;
  unsigned mute_mask = 0;
};
}
