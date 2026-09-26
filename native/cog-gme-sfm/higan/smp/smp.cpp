// GPL-3.0-only. Per-stream SFM scheduler; reset register values from higan v095.
#include "smp.hpp"
#include <algorithm>
#define SMP_CPP
namespace SuperFamicom {
#include "memory.cpp"
#include "timing.cpp"

void SMP::step(unsigned cycles) {
  clock += cycles;
  dsp.clock -= static_cast<int64_t>(cycles) * dsp_clock_step;
}
void SMP::synchronize_dsp() { while(dsp.clock < 0) dsp.enter(); }
void SMP::enter() {
  while(remaining) {
    clock -= remaining * 384;
    while(clock < 0 && status.clock_speed != 2 && !halted) op_step();
    if(clock < 0) step(static_cast<unsigned>(-clock));
    synchronize_dsp();
  }
}
void SMP::render(int16_t* buffer, unsigned count) {
  while(count) {
    const unsigned chunk = std::min(count, 4096u);
    output = buffer;
    remaining = chunk;
    enter();
    if(buffer) buffer += chunk;
    count -= chunk;
  }
}
void SMP::skip(unsigned count) { render(nullptr, count); }
bool SMP::sample(int16_t left, int16_t right) {
  if(remaining < 2) return false;
  if(output) { *output++ = left; *output++ = right; }
  remaining -= 2;
  return true;
}
uint8_t SMP::read_logged_port(unsigned port) {
  if(queue && queue < queue_end) {
    sfm_last[port] = *queue++;
    if(queue == queue_end) queue = queue_loop;
  }
  return sfm_last[port];
}
void SMP::power() {
  timer0.target = timer1.target = timer2.target = 0;
  dsp.power();
  reset();
}
SMP::SMP() : dsp(*this), timer0(*this), timer1(*this), timer2(*this) {
  for(auto& byte : iplrom) byte = 0;
}
SMP::~SMP() = default;
void SMP::reset() {
  clock = 0;
  halted = false;
  dsp.reset();

  regs.pc = 0xffc0;
  regs.a = 0x00;
  regs.x = 0x00;
  regs.y = 0x00;
  regs.s = 0xef;
  regs.p = 0x02;

  for(auto& n : apuram) n = 0;
  apuram[0x00f4] = 0x00;
  apuram[0x00f5] = 0x00;
  apuram[0x00f6] = 0x00;
  apuram[0x00f7] = 0x00;

  status.clock_counter = 0;
  status.dsp_counter = 0;
  status.timer_step = 3;

  //$00f0
  status.clock_speed = 0;
  status.timer_speed = 0;
  status.timers_enable = true;
  status.ram_disable = false;
  status.ram_writable = true;
  status.timers_disable = false;

  //$00f1
  status.iplrom_enable = true;

  //$00f2
  status.dsp_addr = 0x00;

  //$00f8,$00f9
  status.ram00f8 = 0x00;
  status.ram00f9 = 0x00;

  timer0.stage0_ticks = 0;
  timer1.stage0_ticks = 0;
  timer2.stage0_ticks = 0;

  timer0.stage1_ticks = 0;
  timer1.stage1_ticks = 0;
  timer2.stage1_ticks = 0;

  timer0.stage2_ticks = 0;
  timer1.stage2_ticks = 0;
  timer2.stage2_ticks = 0;

  timer0.stage3_ticks = 0;
  timer1.stage3_ticks = 0;
  timer2.stage3_ticks = 0;

  timer0.current_line = 0;
  timer1.current_line = 0;
  timer2.current_line = 0;

  timer0.enable = false;
  timer1.enable = false;
  timer2.enable = false;
}

}
