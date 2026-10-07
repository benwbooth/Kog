/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later */
#include "iop_interpreter.h"
#include "COP_SCU.h"
#include <cstdio>
#include <cstring>
#include <limits>
#include <stdexcept>

namespace {
int32 signed32(uint32 value) { int32 result; std::memcpy(&result, &value, sizeof(result)); return result; }
uint32 arithmeticRight(uint32 value, unsigned shift)
{
    shift &= 31;
    if(!shift) return value;
    return (value >> shift) | ((value & 0x80000000U) ? (~uint32(0) << (32 - shift)) : 0);
}
}

KogIopInterpreter::KogIopInterpreter(CMIPS& cpu, bool ps2Mode) : m_cpu(cpu), m_ps2Mode(ps2Mode) {}
void KogIopInterpreter::Reset()
{
    m_loadRegister = m_nextLoadRegister = m_writtenRegister = 0;
    m_loadValue = m_nextLoadValue = 0;
    m_delayedTargetIsOne = false;
    m_fetchStart = 1;
    m_fetchEnd = 0;
    m_fetchBase = nullptr;
    m_readRegion = m_writeRegion = Region();
#ifdef DEBUGGER_INCLUDED
    m_break = m_ignoreBreakpoint = false;
#endif
}
uint32 KogIopInterpreter::reg(unsigned index) const { return index ? m_cpu.m_State.nGPR[index].nV0 : 0; }
void KogIopInterpreter::write(unsigned index, uint32 value)
{
    if(!index) return;
    m_cpu.m_State.nGPR[index].nD0 = static_cast<int64>(signed32(value));
    m_writtenRegister = index;
}
void KogIopInterpreter::load(unsigned index, uint32 value)
{
    if(m_exception) return;
    m_nextLoadRegister = index;
    m_nextLoadValue = value;
}
void KogIopInterpreter::PrepareInterrupt()
{
    if(m_loadRegister) write(m_loadRegister, m_loadValue);
    m_loadRegister = 0;
}
uint32 KogIopInterpreter::address(uint32 value) const
{
    // Play!'s translator only masks to the physical range; skip the indirect call.
    if(m_cpu.m_pAddrTranslator == &CMIPS::TranslateAddress64) return value & 0x1FFFFFFFU;
    return m_cpu.m_pAddrTranslator ? m_cpu.m_pAddrTranslator(&m_cpu, value) : value;
}
/// Host memory for `width` bytes at `physical` when it lies in a plain-memory
/// region (the same storage CMemoryMap reads and writes), else null.
uint8* KogIopInterpreter::region(Region& cache, const CMemoryMap::MEMORYMAPELEMENT* (CMemoryMap::*lookup)(uint32) const,
                                 uint32 physical, unsigned width)
{
    if(physical < cache.start || uint64(physical) + width - 1 > cache.end)
    {
        auto* map = m_cpu.m_pMemoryMap;
        const auto* element = (map->*lookup)(physical);
        // Only cache a region that owns its whole range, so a cached hit is
        // exactly what the memory map's first-match search would return.
        if(!element || element->nType != CMemoryMap::MEMORYMAP_TYPE_MEMORY ||
           uint64(physical) + width - 1 > element->nEnd ||
           (map->*lookup)(element->nStart) != element || (map->*lookup)(element->nEnd) != element)
            return nullptr;
        cache = Region{element->nStart, element->nEnd, static_cast<uint8*>(element->pPointer)};
    }
    return cache.base + (physical - cache.start);
}
uint32 KogIopInterpreter::read(uint32 value, unsigned width)
{
    if(value & (width - 1)) { exception(4, value); return 0; }
    const uint32 physical = address(value);
    auto* map = m_cpu.m_pMemoryMap;
    // Cache-control reads do not touch the host. This is the R3000 BIU register.
    if(value == 0xfffe0130U) return 0;
    if(const uint8* host = region(m_readRegion, &CMemoryMap::GetReadMap, physical, width))
    {
        if(width == 1) return *host;
        if(width == 2) { uint16 half; std::memcpy(&half, host, 2); return half; }
        uint32 word; std::memcpy(&word, host, 4); return word;
    }
    if(width == 1) return map->GetByte(physical);
    if(width == 2) return map->GetHalf(physical);
    return map->GetWord(physical);
}
void KogIopInterpreter::store(uint32 value, unsigned width, uint32 data)
{
    if(value & (width - 1)) { exception(5, value); return; }
    if(value == 0xfffe0130U) return;
    // Isolated cache writes must not corrupt the backing RAM during cache flushes.
    if((m_cpu.m_State.nCOP0[CCOP_SCU::STATUS] & (1U << 16)) && value < 0xa0000000U) return;
    auto* map = m_cpu.m_pMemoryMap;
    const uint32 physical = address(value);
    if(uint8* host = region(m_writeRegion, &CMemoryMap::GetWriteMap, physical, width))
    {
        if(width == 1) *host = static_cast<uint8>(data);
        else if(width == 2) { const uint16 half = static_cast<uint16>(data); std::memcpy(host, &half, 2); }
        else std::memcpy(host, &data, 4);
        return;
    }
    if(width == 1) map->SetByte(physical, static_cast<uint8>(data));
    else if(width == 2) map->SetHalf(physical, static_cast<uint16>(data));
    else map->SetWord(physical, data);
}
void KogIopInterpreter::exception(unsigned code, uint32 badAddress)
{
    auto& s = m_cpu.m_State;
    s.nCOP0[CCOP_SCU::CAUSE] = (s.nCOP0[CCOP_SCU::CAUSE] & 0x0000ff00U) |
                             (code << 2) | (m_delaySlot ? 0x80000000U : 0);
    s.nCOP0[CCOP_SCU::EPC] = m_pc - (m_delaySlot ? 4 : 0);
    if(code == 4 || code == 5) s.nCOP0[CCOP_SCU::BADVADDR] = badAddress;
    const uint32 status = s.nCOP0[CCOP_SCU::STATUS];
    s.nCOP0[CCOP_SCU::STATUS] = (status & ~0x3fU) | ((status << 2) & 0x3fU);
    s.nPC = (status & (1U << 22)) ? 0xbfc00180U : 0x80000080U;
    s.nDelayedJumpAddr = MIPS_INVALID_PC;
    m_exception = true;
}
void KogIopInterpreter::hleCall()
{
    if(m_ps2Mode && (m_cpu.m_pMemoryMap->GetInstruction(address(m_pc)) >> 26) == 9)
    {
        // Upstream scans backward to the IRX import marker without a bound.
        // Validate the marker before entering that HLE path on untrusted code.
        const uint32 physical = address(m_pc);
        const auto* map = m_cpu.m_pMemoryMap->GetReadMap(physical);
        bool found = false;
        if(map && map->nType == CMemoryMap::MEMORYMAP_TYPE_MEMORY)
        {
            const uint32 lower = physical - map->nStart > 0x100000 ? physical - 0x100000 : map->nStart;
            for(uint32 pos = physical; ; pos -= 4)
            {
                if(m_cpu.m_pMemoryMap->GetWord(pos) == 0x41e00000U && uint64(pos) + 19 <= map->nEnd)
                { found = true; break; }
                if(pos < lower + 4) break;
            }
        }
        if(!found) throw std::runtime_error("PSF2 import trap has no bounded IRX import record");
    }
    // This is Play!'s explicit BIOS ABI, not a hardware exception vector. Import
    // stubs use ADDIU zero,zero,ordinal in a JR delay slot, so keep the resolved PC.
    m_cpu.m_State.nCOP0[CCOP_SCU::EPC] = m_pc;
    m_cpu.m_State.nHasException = MIPS_EXCEPTION_SYSCALL;
}
int KogIopInterpreter::Execute(int quota)
{
    auto& s = m_cpu.m_State;
    s.cycleQuota = quota;
#ifdef DEBUGGER_INCLUDED
    m_break = false;
#endif
    while(s.cycleQuota > 0 && !s.nHasException)
    {
        m_pc = s.nPC;
#ifdef DEBUGGER_INCLUDED
        if(!m_ignoreBreakpoint && m_cpu.m_breakpoints.count(m_pc)) { m_break = true; break; }
        m_ignoreBreakpoint = false;
#endif
        m_delaySlot = s.nDelayedJumpAddr != MIPS_INVALID_PC;
        const uint32 next = m_delaySlot ? (m_delayedTargetIsOne ? 1 : s.nDelayedJumpAddr) : m_pc + 4;
        m_delayedTargetIsOne = false;
        s.nDelayedJumpAddr = MIPS_INVALID_PC;
        m_exception = false;
        m_writtenRegister = m_nextLoadRegister = 0;
        if(m_pc & 3) exception(4, m_pc);
        else
        {
            const uint32 physical = address(m_pc);
            if(physical < m_fetchStart || uint64(physical) + 3 > m_fetchEnd)
            {
                const auto* mapping = m_cpu.m_pMemoryMap->GetInstructionMap(physical);
                if(!mapping || uint64(physical) + 3 > mapping->nEnd)
                {
                    char message[128];
                    std::snprintf(message, sizeof(message), "PSF interpreter fetched unmapped code at 0x%08x", m_pc);
                    throw std::runtime_error(message);
                }
                if(mapping->nType != CMemoryMap::MEMORYMAP_TYPE_MEMORY)
                {
                    step(m_cpu.m_pMemoryMap->GetInstruction(physical));
                    goto retire;
                }
                m_fetchStart = mapping->nStart;
                m_fetchEnd = mapping->nEnd;
                m_fetchBase = static_cast<const uint8*>(mapping->pPointer);
            }
            {
                // Same little-endian word read as CMemoryMap_LSBF::GetInstruction.
                uint32 instruction;
                std::memcpy(&instruction, m_fetchBase + (physical - m_fetchStart), sizeof(instruction));
                step(instruction);
            }
        retire:;
        }
        // A following ALU write or a second load to the same register wins. LWL/
        // LWR below explicitly forward the pending value to merge paired loads.
        if(m_loadRegister && m_loadRegister != m_writtenRegister &&
           m_loadRegister != m_nextLoadRegister)
            s.nGPR[m_loadRegister].nD0 = static_cast<int64>(signed32(m_loadValue));
        m_loadRegister = m_exception ? 0 : m_nextLoadRegister;
        m_loadValue = m_nextLoadValue;
        if(!m_exception) s.nPC = next;
        s.nGPR[0] = {};
        --s.cycleQuota;
        // HLE callbacks can inspect every register and switch guest threads.
        if(s.nHasException) PrepareInterrupt();
    }
    return s.cycleQuota;
}
void KogIopInterpreter::step(uint32 ins)
{
    auto& s = m_cpu.m_State;
    const unsigned op = ins >> 26, rs = (ins >> 21) & 31, rt = (ins >> 16) & 31;
    const unsigned rd = (ins >> 11) & 31, sh = (ins >> 6) & 31, fn = ins & 63;
    const uint32 a = reg(rs), b = reg(rt);
    const uint32 imm = static_cast<uint32>(static_cast<int32>(static_cast<int16>(ins)));
    const uint32 ea = a + imm;
    const auto branch = [&](bool take) { s.nDelayedJumpAddr = take ? m_pc + 4 + imm * 4 : m_pc + 8; };
    const auto jumpRegister = [&]() { m_delayedTargetIsOne = a == MIPS_INVALID_PC; s.nDelayedJumpAddr = m_delayedTargetIsOne ? 0 : a; };
    const auto add = [&](unsigned dest, uint32 x, uint32 y, bool trap) {
        const uint32 result = x + y;
        if(trap && ((~(x ^ y) & (x ^ result)) >> 31)) exception(12);
        else write(dest, result);
    };
    switch(op)
    {
    case 0:
        switch(fn)
        {
        case 0x00: write(rd, b << sh); break;
        case 0x02: write(rd, b >> sh); break;
        case 0x03: write(rd, arithmeticRight(b, sh)); break;
        case 0x04: write(rd, b << (a & 31)); break;
        case 0x06: write(rd, b >> (a & 31)); break;
        case 0x07: write(rd, arithmeticRight(b, a)); break;
        case 0x08: jumpRegister(); break;
        case 0x09: write(rd, m_pc + 8); jumpRegister(); break;
        case 0x0a: if(!b) write(rd, a); break; // IOP code also uses MIPS-II/IV moves
        case 0x0b: if(b) write(rd, a); break;
        case 0x0c: hleCall(); break;
        case 0x0d: exception(9); break;
        case 0x0f: break; // SYNC has no outstanding host memory operations
        case 0x10: write(rd, s.nHI[0]); break;
        case 0x11: s.nHI[0] = a; s.nHI[1] = arithmeticRight(a, 31); break;
        case 0x12: write(rd, s.nLO[0]); break;
        case 0x13: s.nLO[0] = a; s.nLO[1] = arithmeticRight(a, 31); break;
        case 0x18:
        case 0x19:
        {
            const uint64 product = fn == 0x18 ? static_cast<uint64>(int64(signed32(a)) * int64(signed32(b))) : uint64(a) * b;
            s.nLO[0] = static_cast<uint32>(product); s.nHI[0] = static_cast<uint32>(product >> 32);
            s.nLO[1] = arithmeticRight(s.nLO[0], 31); s.nHI[1] = arithmeticRight(s.nHI[0], 31);
            break;
        }
        case 0x1a:
            if(!b) { s.nLO[0] = signed32(a) < 0 ? 1 : 0xffffffffU; s.nHI[0] = a; }
            else if(a == 0x80000000U && b == 0xffffffffU) { s.nLO[0] = a; s.nHI[0] = 0; }
            else { s.nLO[0] = static_cast<uint32>(signed32(a) / signed32(b)); s.nHI[0] = static_cast<uint32>(signed32(a) % signed32(b)); }
            s.nLO[1] = arithmeticRight(s.nLO[0], 31); s.nHI[1] = arithmeticRight(s.nHI[0], 31);
            break;
        case 0x1b:
            s.nLO[0] = b ? a / b : 0xffffffffU; s.nHI[0] = b ? a % b : a;
            s.nLO[1] = arithmeticRight(s.nLO[0], 31); s.nHI[1] = arithmeticRight(s.nHI[0], 31);
            break;
        case 0x20: add(rd, a, b, true); break;
        case 0x21: add(rd, a, b, false); break;
        case 0x22:
            if(((a ^ b) & (a ^ (a - b))) >> 31) exception(12); else write(rd, a - b);
            break;
        case 0x23: write(rd, a - b); break;
        case 0x24: write(rd, a & b); break;
        case 0x25: write(rd, a | b); break;
        case 0x26: write(rd, a ^ b); break;
        case 0x27: write(rd, ~(a | b)); break;
        case 0x2a: write(rd, signed32(a) < signed32(b)); break;
        case 0x2b: write(rd, a < b); break;
        default: exception(10); break;
        }
        break;
    case 1:
        if(rt == 0 || rt == 1 || rt == 16 || rt == 17)
        {
            if(rt & 16) write(31, m_pc + 8);
            branch((signed32(a) >= 0) == ((rt & 1) != 0));
        }
        else exception(10);
        break;
    case 2: s.nDelayedJumpAddr = ((m_pc + 4) & 0xf0000000U) | ((ins & 0x03ffffffU) << 2); break;
    case 3: write(31, m_pc + 8); s.nDelayedJumpAddr = ((m_pc + 4) & 0xf0000000U) | ((ins & 0x03ffffffU) << 2); break;
    case 4: branch(a == b); break;
    case 5: branch(a != b); break;
    case 6: branch(signed32(a) <= 0); break;
    case 7: branch(signed32(a) > 0); break;
    case 8: add(rt, a, imm, true); break;
    case 9: if(m_ps2Mode && !rs && !rt) hleCall(); else add(rt, a, imm, false); break;
    case 10: write(rt, signed32(a) < signed32(imm)); break;
    case 11: write(rt, a < imm); break;
    case 12: write(rt, a & (ins & 0xffff)); break;
    case 13: write(rt, a | (ins & 0xffff)); break;
    case 14: write(rt, a ^ (ins & 0xffff)); break;
    case 15: write(rt, ins << 16); break;
    case 16: cop0(ins); break;
    case 17: case 18: case 19: case 49: case 50: case 51: case 57: case 58: case 59:
        // Play!'s PSF IOP has no FPU/GTE attached. Never silently produce fake results.
        throw std::runtime_error("PSF requests an unavailable R3000 coprocessor (FPU/GTE)");
    case 32: load(rt, static_cast<uint32>(static_cast<int32>(static_cast<int8>(read(ea, 1))))); break;
    case 33: load(rt, static_cast<uint32>(static_cast<int32>(static_cast<int16>(read(ea, 2))))); break;
    case 34:
    {
        const unsigned shift = (3 - (ea & 3)) * 8;
        const uint32 old = m_loadRegister == rt ? m_loadValue : b;
        const uint32 mask = shift ? ((uint32(1) << shift) - 1) : 0;
        load(rt, (old & mask) | (read(ea & ~3U, 4) << shift)); break;
    }
    case 35: load(rt, read(ea, 4)); break;
    case 36: load(rt, read(ea, 1)); break;
    case 37: load(rt, read(ea, 2)); break;
    case 38:
    {
        const unsigned shift = (ea & 3) * 8;
        const uint32 old = m_loadRegister == rt ? m_loadValue : b;
        const uint32 mask = shift ? (0xffffffffU << (32 - shift)) : 0;
        load(rt, (old & mask) | (read(ea & ~3U, 4) >> shift)); break;
    }
    case 40: store(ea, 1, b); break;
    case 41: store(ea, 2, b); break;
    case 42:
    {
        const unsigned shift = (3 - (ea & 3)) * 8;
        const uint32 mask = shift ? (0xffffffffU << (32 - shift)) : 0;
        store(ea & ~3U, 4, (read(ea & ~3U, 4) & mask) | (b >> shift)); break;
    }
    case 43: store(ea, 4, b); break;
    case 46:
    {
        const unsigned shift = (ea & 3) * 8;
        const uint32 mask = shift ? ((uint32(1) << shift) - 1) : 0;
        store(ea & ~3U, 4, (read(ea & ~3U, 4) & mask) | (b << shift)); break;
    }
    case 47: break; // CACHE: interpreter fetches the current bytes on every instruction
    default: exception(10); break;
    }
}
void KogIopInterpreter::cop0(uint32 ins)
{
    auto& s = m_cpu.m_State;
    const unsigned rs = (ins >> 21) & 31, rt = (ins >> 16) & 31, rd = (ins >> 11) & 31;
    if(rs == 0)
    {
        load(rt, s.nCOP0[rd]);
    }
    else if(rs == 4)
    {
        if(rd == CCOP_SCU::CAUSE) s.nCOP0[rd] = (s.nCOP0[rd] & ~0x300U) | (reg(rt) & 0x300U);
        else if(rd != 15) s.nCOP0[rd] = reg(rt); // processor ID is read-only
    }
    else if(rs == 16 && (ins & 63) == 16) // RFE (R3000)
    {
        s.nCOP0[CCOP_SCU::STATUS] = (s.nCOP0[CCOP_SCU::STATUS] & ~0x0fU) |
                                  ((s.nCOP0[CCOP_SCU::STATUS] >> 2) & 0x0fU);
    }
    else exception(10);
}
