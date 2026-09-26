/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later */
#include "iop_interpreter.h"
#include "MA_MIPSIV.h"
#include "COP_SCU.h"
#include "GenericMipsExecutor.h"
#include <array>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <random>
#include <stdexcept>
#include <string>
#include <vector>

static void require(bool condition, const char* message)
{
    if(!condition) throw std::runtime_error(message);
}
static uint32 R(unsigned fn, unsigned rd, unsigned rs, unsigned rt, unsigned sh = 0)
{ return (rs << 21) | (rt << 16) | (rd << 11) | (sh << 6) | fn; }
static uint32 I(unsigned op, unsigned rt, unsigned rs, uint16 imm)
{ return (op << 26) | (rs << 21) | (rt << 16) | imm; }
struct Machine
{
    CMIPS cpu{MEMORYMAP_ENDIAN_LSBF};
    CMA_MIPSIV arch{MIPS_REGSIZE_32};
    CCOP_SCU cop{MIPS_REGSIZE_32};
    alignas(16) std::array<uint8, 65536> ram{};
    explicit Machine(bool jit = false)
    {
        cpu.m_pMemoryMap->InsertReadMap(0, 65535, ram.data(), 1);
        cpu.m_pMemoryMap->InsertWriteMap(0, 65535, ram.data(), 1);
        cpu.m_pMemoryMap->InsertInstructionMap(0, 65535, ram.data(), 1);
        cpu.m_pArch = &arch; cpu.m_pCOP[0] = &cop;
        cpu.m_pAddrTranslator = &CMIPS::TranslateAddress64;
        if(jit) cpu.m_executor = std::make_unique<CGenericMipsExecutor<BlockLookupOneWay>>(cpu, 65536, BLOCK_CATEGORY_PS2_IOP);
        else cpu.m_executor = std::make_unique<KogIopInterpreter>(cpu, false);
        cpu.m_executor->Reset();
        cpu.m_State.nPC = 0x1000;
    }
    void program(std::initializer_list<uint32> code) { std::memcpy(ram.data() + 0x1000, code.begin(), code.size() * 4); }
    uint32 reg(unsigned r) const { return cpu.m_State.nGPR[r].nV0; }
    void set(unsigned r, uint32 v) { cpu.m_State.nGPR[r].nV0 = v; }
    void run(unsigned n) { cpu.m_executor->Execute(static_cast<int>(n)); }
    void word(unsigned a, uint32 v) { std::memcpy(ram.data() + a, &v, 4); }
    uint32 word(unsigned a) const { uint32 v; std::memcpy(&v, ram.data() + a, 4); return v; }
};
static void semanticTests()
{
    {
        Machine m; m.set(1, 0x8000); m.set(2, 7); m.word(0x8000, 99);
        m.program({I(35,2,1,0),R(0x21,3,2,0),R(0x21,4,2,0)});
        m.run(1); require(m.reg(2) == 7, "load becomes visible too early");
        m.run(1); require(m.reg(2) == 99 && m.reg(3) == 7, "load delay across Execute calls");
        m.run(1); require(m.reg(4) == 99, "load never became visible");
    }
    {
        Machine m; m.set(1,0x8000); m.word(0x8000,99);
        m.program({I(35,2,1,0),I(9,2,0,17),0}); m.run(3);
        require(m.reg(2) == 17, "ALU overwrite of pending load");
    }
    {
        Machine m; m.set(1,0x8000); m.word(0x8000,99); m.word(0x8004,100); m.set(2,7);
        m.program({I(35,2,1,0),I(35,2,1,4),R(0x21,3,2,0),0}); m.run(3);
        require(m.reg(3) == 7 && m.reg(2) == 100, "consecutive loads to same register");
    }
    {
        Machine m; m.program({I(4,0,0,2),I(9,2,0,3),I(9,2,0,9),I(9,3,0,5)});
        m.run(1); require(m.cpu.m_State.nPC == 0x1004 && m.cpu.m_State.nDelayedJumpAddr == 0x100c, "branch delay state");
        m.run(2); require(m.reg(2) == 3 && m.reg(3) == 5, "taken branch/delay slot");
    }
    {
        Machine m; m.set(1,1); m.program({I(4,1,0,2),I(9,2,0,3),I(9,3,0,9)}); m.run(3);
        require(m.reg(2) == 3 && m.reg(3) == 9, "untaken branch");
    }
    {
        Machine m; m.set(1,0x1020); m.program({R(9,1,1,0),I(9,2,0,7)}); m.run(2);
        require(m.reg(1) == 0x1008 && m.cpu.m_State.nPC == 0x1020, "JALR overlapping source/link");
    }
    {
        Machine m; m.set(1,1); m.program({R(8,0,1,0),I(9,2,0,7)}); m.run(3);
        require(m.reg(2)==7 && m.cpu.m_State.nCOP0[14]==1 && m.cpu.m_State.nCOP0[8]==1,
                "JR target equal to Play invalid-PC sentinel must still fault after delay slot");
    }
    {
        Machine m; m.set(1,0x8000);m.word(0x8000,123);m.program({I(35,2,1,0),I(35,2,1,1)});m.run(2);
        require(m.reg(2)==123 && m.cpu.m_State.nCOP0[8]==0x8001,"faulting load lost previous pending load");
    }
    for(unsigned offset=0;offset<4;++offset)
    {
        Machine m; m.set(1,0x8000+offset);
        for(unsigned i=0;i<16;++i) m.ram[0x8000+i]=static_cast<uint8>(0x31+i);
        m.program({I(34,2,1,3),I(38,2,1,0),0}); m.run(3);
        uint32 expected; std::memcpy(&expected,m.ram.data()+0x8000+offset,4);
        require(m.reg(2)==expected,"paired little-endian LWL/LWR");
        m.cpu.m_State.nPC=0x1000; m.program({I(42,2,1,3),I(46,2,1,0)});
        m.set(2,0xa1b2c3d4); m.run(2);
        std::memcpy(&expected,m.ram.data()+0x8000+offset,4);
        require(expected==0xa1b2c3d4,"paired SWL/SWR");
        if(offset) require(m.ram[0x8000+offset-1]==0x30+offset,"unaligned store clobbered prefix");
        require(m.ram[0x8004+offset]==0x35+offset,"unaligned store clobbered suffix");
    }
    {
        Machine m; m.set(1,0x80000000); m.set(2,0xffffffff);
        m.program({R(0x1a,0,1,2),R(0x12,3,0,0),R(0x10,4,0,0)}); m.run(3);
        require(m.reg(3)==0x80000000 && m.reg(4)==0,"signed division overflow");
    }
    for(uint32 value : {0U,1U,0xffffffffU,0x80000000U})
    {
        Machine m; m.set(1,value); m.program({R(0x1a,0,1,0),R(0x12,2,0,0),R(0x10,3,0,0)}); m.run(3);
        require(m.reg(2)==((value>>31)?1U:0xffffffffU)&&m.reg(3)==value,"signed division by zero");
    }
    {
        Machine m; m.set(1,0x7fffffff); m.program({I(4,0,0,2),I(8,2,1,1)}); m.run(2);
        require(m.cpu.m_State.nPC==0x80000080 && m.cpu.m_State.nCOP0[14]==0x1000 &&
                m.cpu.m_State.nCOP0[13]==0x80000030,"overflow exception in branch delay slot");
    }
    {
        Machine m; m.set(1,0x8001); m.set(2,17); m.program({I(35,2,1,0)}); m.run(1);
        require(m.reg(2)==17 && m.cpu.m_State.nCOP0[8]==0x8001 &&
                m.cpu.m_State.nCOP0[13]==16,"misaligned LW exception");
    }
    {
        Machine m; m.set(1,0x3f); m.program({I(16,1,4,12<<11),I(16,2,0,12<<11),0,0x42000010}); m.run(4);
        require(m.reg(2)==0x3f && m.cpu.m_State.nCOP0[12]==0x3f,"MTC0/MFC0/RFE");
    }
    {
        Machine m; m.program({I(9,0,0,0),0}); m.run(2); require(!m.cpu.m_State.nHasException,"PS1 ADDIU zero is not an import trap");
    }
    {
        Machine m; m.program({0xc}); m.run(1); require(m.cpu.m_State.nCOP0[14]==0x1000 && m.cpu.m_State.nHasException==MIPS_EXCEPTION_SYSCALL && m.cpu.m_State.nPC==0x1004,"HLE syscall continuation");
    }
    {
        Machine m; m.program({I(35,2,1,0)}); m.set(1,0x8000); m.word(0x8000,123); m.run(1);
        static_cast<KogIopInterpreter*>(m.cpu.m_executor.get())->PrepareInterrupt();
        require(m.reg(2)==123,"interrupt commits pending load");
    }
}
static void differentialTests()
{
    std::mt19937 rng(0x4b4f4750);
    for(unsigned trial=0;trial<100;++trial)
    {
        Machine interpreted, jit(true);
        for(unsigned r=1;r<28;++r) { const uint32 v=rng(); interpreted.set(r,v); jit.set(r,v); }
        interpreted.set(28,0x8000); jit.set(28,0x8000);
        for(unsigned a=0x8000;a<0x8100;a+=4) {const uint32 v=rng(); interpreted.word(a,v); jit.word(a,v);}
        std::vector<uint32> code;
        for(unsigned n=0;n<200;++n)
        {
            unsigned rd=1+rng()%27, rs=1+rng()%27, rt=1+rng()%27;
            const unsigned alu[]={0x21,0x23,0x24,0x25,0x26,0x27,0x2a,0x2b,0x04,0x06,0x07};
            switch(rng()%8)
            {
            case 0: code.push_back(R(alu[rng()%11],rd,rs,rt)); break;
            case 1: { const unsigned ops[]={9,10,11,12,13,14,15}; code.push_back(I(ops[rng()%7],rt,rs,static_cast<uint16>(rng()))); break; }
            case 2: {const unsigned f[]={0,2,3};code.push_back(R(f[rng()%3],rd,0,rt,rng()%32));break;}
            case 3: code.push_back(R((rng()%2)?0x18:0x19,0,rs,rt)); code.push_back(R(0x10,rd,0,0)); code.push_back(R(0x12,rt,0,0));break;
            case 4: code.push_back(I(43,rt,28,static_cast<uint16>((rng()%64)*4)));break;
            case 5: code.push_back(I(35,rt,28,static_cast<uint16>((rng()%64)*4)));code.push_back(0);break;
            case 6: code.push_back(R((rng()%2)?0x1a:0x1b,0,rs,rt));code.push_back(R(0x12,rd,0,0));code.push_back(R(0x10,rt,0,0));break;
            case 7: code.push_back(I((rng()%2)?4:5,rt,rs,2));code.push_back(I(9,rd,rd,1));code.push_back(I(9,rd,rd,2));break;
            }
        }
        code.push_back(0xc);
        std::memcpy(interpreted.ram.data()+0x1000,code.data(),code.size()*4);
        std::memcpy(jit.ram.data()+0x1000,code.data(),code.size()*4);
        interpreted.run(2000); jit.run(2000);
        for(unsigned r=0;r<32;++r)
            if(interpreted.reg(r)!=jit.reg(r)) throw std::runtime_error("JIT differential register mismatch trial "+std::to_string(trial)+" reg "+std::to_string(r));
        require(interpreted.cpu.m_State.nHI[0]==jit.cpu.m_State.nHI[0] && interpreted.cpu.m_State.nLO[0]==jit.cpu.m_State.nLO[0],"JIT differential HI/LO mismatch");
        require(interpreted.ram==jit.ram,"JIT differential RAM mismatch");
    }
}
int main(int argc,char**)
{
    try { semanticTests(); if(argc>1) differentialTests(); std::puts(argc>1 ? "R3000 semantic tests and 100 x 200-operation JIT differential passed" : "R3000 semantic tests passed"); }
    catch(const std::exception& e) {std::fprintf(stderr,"%s\n",e.what());return 1;}
}
