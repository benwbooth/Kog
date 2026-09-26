// higan v095 SMP interface adapted for independent SFM streams (GPLv3).
#pragma once
#include "../../gme/blargg_common.h"
#include "../processor/spc700/spc700.hpp"
#include "../dsp/dsp.hpp"
namespace SuperFamicom {
struct SMP : Processor::SPC700 {
  uint8_t iplrom[64];
  uint8_t apuram[64 * 1024];

  inline void step(unsigned clocks);
  inline void synchronize_dsp();

  uint8_t port_read(uint8_t port) const;
  void port_write(uint8_t port, uint8_t data);

  void enter();
  void power();
  void reset();

  int64_t clock = 0;
  int64_t dsp_clock_step = 4096;
  DSP dsp;
  uint8_t sfm_last[4] {};
  void render(int16_t* buffer, unsigned samples);
  void skip(unsigned samples);
  bool sample(int16_t left, int16_t right);
  void set_tempo(double speed) { dsp_clock_step = static_cast<int64_t>(4096.0 / speed); }
  void set_sfm_queue(const uint8_t* first, const uint8_t* end, const uint8_t* loop) {
    queue = first; queue_end = end; queue_loop = loop;
    for(auto& port : sfm_last) port = 0;
  }
  const uint8_t* get_sfm_queue() const { return queue; }
  size_t get_sfm_queue_remain() const { return queue ? queue_end - queue : 0; }
  uint8_t read_logged_port(unsigned port);
private:
  const uint8_t *queue = nullptr, *queue_end = nullptr, *queue_loop = nullptr;
  int16_t* output = nullptr;
  unsigned remaining = 0;
public:
  SMP();
  ~SMP();

public:
  struct {
    //timing
    unsigned clock_counter;
    unsigned dsp_counter;
    unsigned timer_step;

    //$00f0
    uint8_t clock_speed;
    uint8_t timer_speed;
    bool timers_enable;
    bool ram_disable;
    bool ram_writable;
    bool timers_disable;

    //$00f1
    bool iplrom_enable;

    //$00f2
    uint8_t dsp_addr;

    //$00f8,$00f9
    uint8_t ram00f8;
    uint8_t ram00f9;
  } status;


  friend class SMPcore;


  //memory.cpp
  uint8_t ram_read(uint16_t addr);
  void ram_write(uint16_t addr, uint8_t data);

  uint8_t op_busread(uint16_t addr);
  void op_buswrite(uint16_t addr, uint8_t data);

  void op_io();
  uint8_t op_read(uint16_t addr);
  void op_write(uint16_t addr, uint8_t data);

  uint8_t disassembler_read(uint16_t addr);

  //timing.cpp
  template<unsigned frequency>
  struct Timer {
    SMP& smp;
    explicit Timer(SMP& owner) : smp(owner) {}
    uint8_t stage0_ticks;
    uint8_t stage1_ticks;
    uint8_t stage2_ticks;
    uint8_t stage3_ticks;
    bool current_line;
    bool enable;
    uint8_t target;

    void tick();
    void synchronize_stage1();
  };

  Timer<192> timer0;
  Timer<192> timer1;
  Timer< 24> timer2;

  inline void add_clocks(unsigned clocks);
  inline void cycle_edge();
};



}
