/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later */
#pragma once
#include "MIPS.h"

// R3000 integer/CP0 interpreter for Play!'s firmware-free IOP/PS1 HLE environment.
// It deliberately contains no native-code generator or executable-memory allocator.
class KogIopInterpreter final : public CMipsExecutor
{
public:
    explicit KogIopInterpreter(CMIPS& cpu, bool ps2Mode = true);
    void Reset() override;
    int Execute(int quota) override;
    void ClearActiveBlocksInRange(uint32, uint32, bool) override {} // no code cache
    void PrepareInterrupt();
#ifdef DEBUGGER_INCLUDED
    bool MustBreak() const override { return m_break; }
    void DisableBreakpointsOnce() override { m_ignoreBreakpoint = true; }
    bool FilterBreakpoint() override { return m_ignoreBreakpoint; }
#endif
private:
    CMIPS& m_cpu;
    bool m_ps2Mode;
    uint32 m_loadRegister = 0, m_loadValue = 0;
    uint32 m_nextLoadRegister = 0, m_nextLoadValue = 0;
    uint32 m_writtenRegister = 0;
    uint32 m_pc = 0;
    bool m_delaySlot = false, m_exception = false;
    bool m_delayedTargetIsOne = false;
    // The memory region the last instruction came from. Code runs from the
    // same RAM region almost always, so fetches skip the memory map search.
    uint32 m_fetchStart = 1, m_fetchEnd = 0;
    const uint8* m_fetchBase = nullptr;
    // Likewise for plain-memory data reads and writes (RAM and scratchpad).
    struct Region { uint32 start = 1, end = 0; uint8* base = nullptr; };
    Region m_readRegion, m_writeRegion;
    uint8* region(Region& cache, const CMemoryMap::MEMORYMAPELEMENT* (CMemoryMap::*lookup)(uint32) const,
                  uint32 physical, unsigned width);
#ifdef DEBUGGER_INCLUDED
    bool m_break = false, m_ignoreBreakpoint = false;
#endif
    uint32 reg(unsigned index) const;
    void write(unsigned index, uint32 value);
    void load(unsigned index, uint32 value);
    void exception(unsigned code, uint32 badAddress = 0);
    void hleCall();
    void step(uint32 instruction);
    void cop0(uint32 instruction);
    uint32 address(uint32 value) const;
    uint32 read(uint32 value, unsigned width);
    void store(uint32 value, unsigned width, uint32 data);
};
