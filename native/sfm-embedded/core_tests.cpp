// SPDX-License-Identifier: GPL-3.0-or-later
#include "higan/smp/smp.hpp"
#include "Bml_Parser.h"
#include <array>
#include <cstdio>
#include <stdexcept>
#include <string>
void require(bool ok, const char* message) { if(!ok) throw std::runtime_error(message); }
int main() {
  try {
    Bml_Parser metadata;
    const char bounded[] = {'x', ':', ' ', '1'};
    metadata.parseDocument(bounded, sizeof(bounded));
    require(std::string(metadata.enumValue("x")) == "1", "BML must honor unterminated length bounds");
    const std::string text = "\nempty:\nname: \xc3\xa9tude\n:value\n";
    metadata.parseDocument(text.data(), text.size());
    require(std::string(metadata.enumValue("name")) == "\xc3\xa9tude", "BML preserves UTF-8 values");
    SuperFamicom::SMP left, right;
    left.power(); right.power();
    const uint8_t log[] = {11, 22, 33};
    left.set_sfm_queue(log, log + 3, log + 1);
    require(left.op_busread(0xf4) == 11, "first logged port");
    require(left.op_busread(0xf5) == 22, "second logged port");
    require(left.op_busread(0xf4) == 33, "third logged port");
    require(left.op_busread(0xf6) == 22, "looped log starts at correct offset");
    require(right.op_busread(0xf4) == 0, "streams must not share port state");
    left.set_sfm_queue(log, log + 3, log + 3);
    for(unsigned i = 0; i < 3; ++i) left.op_busread(0xf4);
    require(left.op_busread(0xf4) == 33, "exhausted log keeps last port value");
    left.op_buswrite(0xf1, 0x10);
    require(left.op_busread(0xf4) == 0, "CONTROL clears latched port values");
    left.timer0.enable = true;
    left.timer0.target = 1;
    left.timer0.stage2_ticks = 0;
    left.timer0.stage3_ticks = 15;
    left.timer0.current_line = true;
    left.timer0.stage1_ticks = 0;
    left.op_buswrite(0xf0, 0x0a);
    require(left.op_busread(0xfd) == 0, "timer output wraps at four bits");
    std::array<int16_t, 1024> pcm {};
    for(uint8_t instruction : {uint8_t(0xef), uint8_t(0xff)}) {
      left.reset();
      left.status.iplrom_enable = false;
      left.regs.pc = 0x200;
      left.apuram[0x200] = instruction;
      left.render(pcm.data(), pcm.size());
      require(left.halted, "SLEEP/STOP must yield to audio rendering");
      left.skip(1024);
      require(left.halted, "skip preserves halted CPU state");
    }
    left.reset();
    left.status.clock_speed = 2;
    left.render(pcm.data(), pcm.size());
    require(left.regs.pc == 0xffc0, "clock-stopped CPU must not execute");
    std::puts("SFM log, state isolation, timer wrap, halt, clock-stop and skip passed");
    return 0;
  } catch(const std::exception& failure) {
    std::fprintf(stderr, "%s\n", failure.what());
    return 1;
  }
}
