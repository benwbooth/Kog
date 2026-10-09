#pragma once
#include "inspection_devices.hpp"
#include "../inspection_opl.h"

namespace KogVgmInspection {
inline void Device::write(unsigned a, unsigned v, unsigned width, unsigned index) {
    ++writes; last_address = a; last_value = v;
    const bool quick = functions[index].funcType & RWF_QUICKWRITE;
    // Port routing follows the corresponding libvgm core's write handler.
    if(type == DEVID_SN76496) {
        if(a & 1) { regs[0xff] = v; return; }
        if(v & 0x80) {
            sn_latch = (v >> 4) & 7;
            sn[sn_latch] = (sn[sn_latch] & 0x3f0) | (v & 15);
        } else if(!(sn_latch & 1) && sn_latch < 6) sn[sn_latch] = (sn[sn_latch] & 15) | ((v & 63) << 4);
        else sn[sn_latch] = v & 15;
        return;
    }
    if(type == DEVID_YMW258) {
        if(quick) { unsigned c = (a >> 3) & 31; if((c & 7) != 7) regs[(c - c / 8) * 8 + (a & 7)] = v; return; }
        if(a == 1) selected_channel = (v & 7) == 7 ? 63 : (v & 31) - (v & 31) / 8;
        else if(a == 2) bank = std::min(v, 7u);
        else if(a == 0 && selected_channel < 28) regs[selected_channel * 8 + bank] = v;
        return;
    }
    if(type == DEVID_RF5C68) {
        if(a == 7) { if(v & 0x40) selected_channel = v & 7; regs[0x100] = v; }
        else if(a == 8) regs[0x101] = v;
        else if(a < 7) regs[selected_channel * 8 + a] = v;
        return;
    }
    if(type == DEVID_C6280) {
        if(a == 0) selected_channel = v & 7;
        else if(a >= 2 && a <= 7) regs[selected_channel * 8 + a] = v;
        else regs[0x100 + (a & 15)] = v;
        return;
    }
    if(type == DEVID_QSOUND && width == 9) {
        if(a < 2) latches[a] = v;
        else if(a == 2) regs[v & 255] = latches[0] * 256 + latches[1];
        return;
    }
    if(type == DEVID_SAA1099) {
        if(a & 1) bank = v & 31; else regs[bank] = v;
        return;
    }
    if(type == DEVID_K051649) {
        if(!(a & 1)) bank = v;
        else regs[((a >> 1) * 256 + bank) & 65535] = v;
        return;
    }
    const bool ports = type == DEVID_YM2413 || type == DEVID_YM2612 || type == DEVID_YM2151 ||
        type == DEVID_YM2203 || type == DEVID_YM2608 || type == DEVID_YM2610 ||
        type == DEVID_YM3812 || type == DEVID_YM3526 || type == DEVID_Y8950 ||
        type == DEVID_YMF262 || type == DEVID_YMF278B || type == DEVID_YMZ280B || type == DEVID_AY8910;
    if(ports && !quick && width == 9) {
        unsigned port = (a >> 1) & 127;
        if(!(a & 1)) { latches[port] = v; return; }
        a = port * 256 + latches[port];
    }
    regs[a & 65535] = v;
    if(type == DEVID_YM2612 && a == 0x2a) dac_write(uint8_t(v));
    if((type == DEVID_YM2612 || type == DEVID_YM2203 || type == DEVID_YM2608 || type == DEVID_YM2610) && a == 0x28) {
        unsigned ch = v & 3; if(ch < 3) keys[ch + ((v & 4) ? 3 : 0)] = v >> 4;
    }
    if(type == DEVID_YM2151 && a == 8) keys[v & 7] = v & 0x78;
    if(type == DEVID_K054539 && (a == 0x214 || a == 0x215))
        for(unsigned ch = 0; ch < 8; ++ch) if(v & (1 << ch)) keys[ch] = a == 0x214;
    if(type == DEVID_C352 && a == 0x202)
        for(unsigned ch = 0; ch < 32; ++ch) {
            auto& flag = regs[ch * 8 + 3];
            if(flag & 0x4000) { keys[ch] = 1; flag &= ~0x4000; }
            else if(flag & 0x2000) { keys[ch] = 0; flag &= ~0x2000; }
        }
}

inline void Device::snapshot(KogVoices& out) const {
    if(linked) return;
    auto emit = [&](unsigned ch, unsigned kind, bool gate, double hz, float level, float pan,
                    unsigned base, unsigned count, double sample_rate = 0.0) -> KogVoice& {
        std::string label = name + " " + (ch < channel_names.size() ? channel_names[ch] : std::to_string(ch + 1));
        auto& v = out.add(label.c_str(), kind, gate, hz, level, pan);
        v.id = id * 256 + ch;
        std::snprintf(v.instrument, sizeof(v.instrument), "%s", kind == 2 ? "PCM / ADPCM" : kind == 1 ? "Noise" : kind == 4 ? "Register state" : "");
        int n = std::snprintf(v.details, sizeof(v.details), "Clock=%u Hz", clock);
        if(hz > 0) n += std::snprintf(v.details + n, sizeof(v.details) - n, " | Frequency=%.3f Hz", hz);
        if(sample_rate > 0) n += std::snprintf(v.details + n, sizeof(v.details) - n, " | Sample rate=%.3f Hz", sample_rate);
        for(unsigned r = 0; r < count && n < int(sizeof(v.details)) - 20; ++r) {
            const unsigned address = (base + r) & 65535;
            n += std::snprintf(v.details + n, sizeof(v.details) - n, " | R%03X=%04X", address, regs[address]);
        }
        return v;
    };
    auto ay = [&](unsigned first, double chip_clock) {
        for(unsigned ch = 0; ch < 3; ++ch) {
            unsigned period = regs[ch*2] | ((regs[ch*2+1] & 15) << 8);
            bool tone = !(regs[7] & (1 << ch)), noise = !(regs[7] & (8 << ch));
            auto& v = emit(first+ch, tone ? 0 : 1, (tone || noise) && (regs[8+ch] & 31),
                tone ? chip_clock / (16.0 * std::max(period, 1u)) : 0, (regs[8+ch] & 16) ? 1 : (regs[8+ch] & 15) / 15.0f, 0, 0, 14);
            std::snprintf(v.instrument, sizeof(v.instrument), "%s%s%s", tone ? "Tone" : "", noise ? " + noise" : "", regs[8+ch]&16 ? " + envelope" : "");
        }
    };
    if(type == DEVID_SN76496) {
        for(unsigned ch = 0; ch < 4; ++ch) {
            auto& v = emit(ch, ch == 3 ? 1 : 0, sn[ch*2+1] < 15,
                ch < 3 ? clock / (32.0 * std::max(unsigned(sn[ch*2]), 1u)) : 0,
                std::pow(10.0f, -float(sn[ch*2+1])/10.0f), 0, 0, 0);
            // The level already is the attenuation (2 dB a step), so it is not
            // repeated as a field.
            std::snprintf(v.details, sizeof(v.details), "Period/control=%03X | Clock=%u Hz | Stereo=%02X", sn[ch*2], clock, regs[0xff]);
        }
        return;
    }
    if(type == DEVID_AY8910) { ay(0, clock); return; }
    if(type == DEVID_YM3812 || type == DEVID_YM3526 || type == DEVID_Y8950 || type == DEVID_YMF262 || type == DEVID_YMF278B) {
        std::array<uint8_t,512> r{}; std::copy_n(regs.begin(), 512, r.begin());
        size_t first = out.count;
        kog_inspect_opl(r.data(), out, name.c_str(), (type == DEVID_YMF262 || type == DEVID_YMF278B) ? clock : clock * 4.0);
        for(size_t i = first; i < out.count; ++i) out.out[i].id += id * 256;
        if(type == DEVID_Y8950) emit(23,2,regs[7]&0x80,0,(regs[0x12]&255)/255.0f,0,7,13);
        if(type == DEVID_YMF278B) for(unsigned ch=0; ch<24; ++ch) {
            auto& v = emit(24+ch,2,regs[0x268+ch]&0x80,0,1-(regs[0x250+ch]>>1)/127.0f,0,0x208+ch,1);
            std::snprintf(v.details,sizeof(v.details),"Wave=%03X | Pitch=%02X %02X | Level=%02X | Key/pan=%02X | LFO=%02X | Envelope=%02X %02X %02X",regs[0x208+ch]|((regs[0x220+ch]&1)<<8),regs[0x220+ch],regs[0x238+ch],regs[0x250+ch],regs[0x268+ch],regs[0x280+ch],regs[0x298+ch],regs[0x2b0+ch],regs[0x2c8+ch]);
        }
        return;
    }
    if(type == DEVID_YM2413) {
        bool rhythm = regs[0xe]&0x20;
        for(unsigned ch=0; ch<(rhythm?6u:9u); ++ch) {
            unsigned f = regs[0x10+ch] | ((regs[0x20+ch]&1)<<8), block=(regs[0x20+ch]>>1)&7;
            auto& v = emit(ch,0,regs[0x20+ch]&16, clock/72.0*f*(1<<block)/524288.0,
                std::pow(10.0f, -(regs[0x30+ch]&15)*3.0f/20),0,0,8);
            std::snprintf(v.instrument,sizeof(v.instrument),"OPLL instrument %u",regs[0x30+ch]>>4);
        }
        if(rhythm) for(unsigned ch=0; ch<5; ++ch) emit(9+ch,3,regs[0xe]&(1<<ch),0,1,0,0xe,1);
        return;
    }
    if(type == DEVID_YM2612 || type == DEVID_YM2203 || type == DEVID_YM2608 || type == DEVID_YM2610) {
        const unsigned count = type == DEVID_YM2203 ? 3 : 6;
        for(unsigned ch=0; ch<count; ++ch) {
            unsigned b = ch/3*256, c=ch%3, f=regs[b+0xa0+c]|((regs[b+0xa4+c]&7)<<8), block=(regs[b+0xa4+c]>>3)&7;
            const bool dac = type == DEVID_YM2612 && ch == 5 && (regs[0x2b]&0x80);
            if(dac) {
                // Each DAC drum sample sits on its own key, sample 1 on C2
                // (MIDI 36), like a drum map, so hits show on the keyboard
                // and in scores; the key is the sample's number, not a pitch.
                dac_frame();
                const bool sounding = dac_sounding && dac_sample > 0;
                const double key = 35.0 + dac_sample;
                auto& v=emit(ch,3,sounding,sounding ? 440.0*std::pow(2.0,(key-69.0)/12.0) : 0,
                    1.0f, ((regs[b+0xb4+c]&0x40)?1.0f:0.0f)-((regs[b+0xb4+c]&0x80)?1.0f:0.0f), 0, 0);
                std::snprintf(v.instrument,sizeof(v.instrument),"DAC drums");
                std::snprintf(v.details,sizeof(v.details),"Sample=%u | Key on=%u | Pitch basis=Drum map (C2 = sample 1)",dac_sample,dac_hits);
                continue;
            }
            auto& v=emit(ch,0,keys[ch],clock/(type==DEVID_YM2203?72.0:144.0)*f*(1<<block)/2097152.0,
                std::pow(10.0f,-float(regs[b+0x4c+c]&127)*0.75f/20), ((regs[b+0xb4+c]&0x40)?1.0f:0.0f)-((regs[b+0xb4+c]&0x80)?1.0f:0.0f), b+0xa0+c,1);
            std::snprintf(v.instrument,sizeof(v.instrument),"%s","4-op FM");
            // Operator 4's level is the channel level above, so the field
            // lists operators 1 to 3.
            int n = std::snprintf(v.details,sizeof(v.details),"F-number=%03X | Block=%u | Key operators=%X | Algorithm/feedback=%02X | Pan/LFO=%02X | Operator 1-3 levels=%02X %02X %02X",f,block,keys[ch],regs[b+0xb0+c],regs[b+0xb4+c],regs[b+0x40+c],regs[b+0x44+c],regs[b+0x48+c]);
            // Register 27h's low bits are timer controls that drivers rewrite
            // constantly; only its top bits (channel 3's mode) shape the sound.
            if(b == 0 && c == 2 && n > 0 && size_t(n) < sizeof(v.details))
                std::snprintf(v.details+n,sizeof(v.details)-n," | Channel 3 mode=%u",regs[0x27]>>6);
        }
        if(type != DEVID_YM2612) ay(6, clock/(type==DEVID_YM2203?2.0:4.0));
        if(type == DEVID_YM2608 || type == DEVID_YM2610) {
            for(unsigned ch=0; ch<6; ++ch) emit(9+ch,3,regs[type==DEVID_YM2608?0x10:0x100]&(1<<ch),0,1,0,type==DEVID_YM2608?0x18+ch:0x108+ch,1);
            emit(15,2,regs[type==DEVID_YM2608?0x100:0x10]&0x80,0,1,0,type==DEVID_YM2608?0x100:0x10,16);
        }
        return;
    }
    if(type == DEVID_YM2151) {
        for(unsigned ch=0; ch<8; ++ch) {
            unsigned kc=regs[0x28+ch], note=(kc&15)-(kc&15)/4;
            double semitones=(int(kc>>4)-2)*12+note+(regs[0x30+ch]>>2)/64.0;
            double hz=1299.0*clock/67108864.0*std::exp2(semitones/12.0);
            auto& v=emit(ch,0,keys[ch],hz,std::pow(10.0f,-float(regs[0x78+ch]&127)*0.75f/20),
                ((regs[0x20+ch]&0x40)?1.0f:0.0f)-((regs[0x20+ch]&0x80)?1.0f:0.0f),0x20+ch,1);
            std::snprintf(v.details,sizeof(v.details),"Key code=%02X | Fraction=%02X | Key operators=%02X | Algorithm/pan=%02X | PMS/AMS=%02X | Levels=%02X %02X %02X %02X | LFO=%02X %02X",kc,regs[0x30+ch],keys[ch],regs[0x20+ch],regs[0x38+ch],regs[0x60+ch],regs[0x68+ch],regs[0x70+ch],regs[0x78+ch],regs[0x18],regs[0x1b]);
        }
        return;
    }
    if(type == DEVID_K051649) {
        for(unsigned ch=0; ch<5; ++ch) { unsigned p=regs[0x100+ch*2]|((regs[0x101+ch*2]&15)<<8);
            emit(ch,0,regs[0x300]&(1<<ch),clock/(32.0*(p+1)),(regs[0x200+ch]&15)/15.0f,0,0x100+ch*2,2); }
        return;
    }
    if(type == DEVID_C6280) {
        for(unsigned ch=0; ch<6; ++ch) { auto r=&regs[ch*8]; unsigned p=r[2]|((r[3]&15)<<8); bool noise=ch>=4 && (r[7]&0x80), dda=r[4]&0x40;
            emit(ch,dda?2:noise?1:0,r[4]&0x80,(!noise&&!dda)?clock/(32.0*std::max(p,1u)):0,(r[4]&31)/31.0f,((r[5]&15)-(r[5]>>4))/15.0f,ch*8+2,6); }
        return;
    }
    if(type == DEVID_SAA1099) {
        for(unsigned ch=0; ch<6; ++ch) { unsigned octave=(regs[0x10+ch/2]>>((ch&1)*4))&7; bool tone=regs[0x14]&(1<<ch),noise=regs[0x15]&(1<<ch);
            emit(ch,tone?0:1,(regs[0x1c]&1)&&(tone||noise),tone?clock/512.0*(1<<octave)/(511-regs[8+ch]):0,std::max(regs[ch]&15,regs[ch]>>4)/15.0f,((regs[ch]>>4)-(regs[ch]&15))/15.0f,8+ch,1); }
        return;
    }
    if(type == DEVID_WSWAN) {
        for(unsigned ch=0; ch<4; ++ch) { unsigned p=regs[0x80+2*ch]|((regs[0x81+2*ch]&7)<<8); bool noise=ch==3&&(regs[0x90]&0x80),pcm=ch==1&&(regs[0x90]&0x20);
            emit(ch,pcm?2:noise?1:0,regs[0x90]&(1<<ch),(!noise&&!pcm)?clock/(32.0*(2048-p)):0,std::max(regs[0x88+ch]&15,regs[0x88+ch]>>4)/15.0f,0,0x80+2*ch,2); }
        return;
    }
    if(type == DEVID_GB_DMG) {
        for(unsigned ch=0;ch<4;++ch) {
            unsigned base=ch==0?0:ch==1?5:ch==2?10:15;
            unsigned p=regs[base+3]|((regs[base+4]&7)<<8);
            bool gate=(regs[0x16]&128)&&(regs[base+4]&128);
            float level=ch==2 ? ((regs[12]>>5)&3 ? std::exp2(1-int((regs[12]>>5)&3)) : 0) : (regs[base+2]>>4)/15.0f;
            if(ch==2) gate=gate&&(regs[10]&128);
            emit(ch,ch==3?1:0,gate,ch<3 ? clock/((ch==2?64.0:32.0)*(2048-p)) : 0,level,
                ((regs[0x15]&(1<<ch))?1.0f:0.0f)-((regs[0x15]&(16<<ch))?1.0f:0.0f),base,5);
        }
        return;
    }
    if(type == DEVID_NES_APU) {
        for(unsigned ch=0;ch<5;++ch) {
            unsigned b=ch*4,p=regs[b+2]|((regs[b+3]&7)<<8);
            bool gate=regs[0x15]&(1<<ch);
            emit(ch,ch==3?1:ch==4?2:0,gate,ch<3 ? clock/((ch==2?32.0:16.0)*(p+1)) : 0,
                ch==2?1.0f:ch==4?(regs[0x11]&127)/127.0f:(regs[b]&15)/15.0f,0,b,4);
        }
        return;
    }
    if(type == DEVID_POKEY) {
        for(unsigned ch=0;ch<4;++ch) {
            unsigned control=regs[8],c=regs[ch*2+1],p=regs[ch*2]+1,div=(control&1)?114:28;
            const bool joined=ch%2 && (control & (ch==1?0x10:8));
            const bool fast=control & (ch<2?0x40:0x20);
            if(joined) p=(regs[ch*2]<<8)|regs[ch*2-2];
            if(fast&&(ch%2==0||joined)) { div=1; p+=joined?7:3; }
            else if(joined) ++p;
            const bool tone=(c&0xa0)==0xa0 && !(c&16);
            emit(ch,tone?0:1,(c&15)!=0,tone?clock/(2.0*p*div):0,(c&15)/15.0f,0,ch*2,2);
        }
        return;
    }
    if(type == DEVID_VBOY_VSU) {
        for(unsigned ch=0;ch<6;++ch) { unsigned b=0x400+ch*64,p=regs[b+8]|((regs[b+12]&7)<<8);
            emit(ch,ch==5?1:0,regs[b]&128,ch<5?clock/(32.0*(2048-p)):0,(regs[b+16]>>4)/15.0f,
                ((regs[b+4]&15)-(regs[b+4]>>4))/15.0f,b,24); }
        return;
    }
    if(type == DEVID_YMZ280B) {
        for(unsigned ch=0;ch<8;++ch) { unsigned b=ch*4,p=regs[b]|((regs[b+1]&1)<<8);
            emit(ch,2,(regs[255]&128)&&(regs[b+1]&128),0,regs[b+2]/255.0f,(int(regs[b+3]&15)-8)/7.0f,b,4,clock/384.0*(p+1)/256); }
        return;
    }
    if(type == DEVID_GA20) { for(unsigned ch=0;ch<4;++ch) { unsigned b=ch*8;
        emit(ch,2,regs[b+6]!=0,0,regs[b+5]/(float(regs[b+5])+10),0,b,8,clock/(4.0*(256-regs[b+4]))); } return; }
    if(type == DEVID_SEGAPCM) { for(unsigned ch=0; ch<16; ++ch) { unsigned b=ch*8;
        emit(ch,2,!(regs[b+0x86]&1),0,std::max(regs[b+2]&127,regs[b+3]&127)/127.0f,(int(regs[b+3])-int(regs[b+2]))/127.0f,b,8,clock/128.0*regs[b+7]/256); } return; }
    if(type == DEVID_RF5C68) { for(unsigned ch=0; ch<8; ++ch) { auto r=&regs[ch*8];
        emit(ch,2,(regs[0x100]&128)&&!(regs[0x101]&(1<<ch)),0,r[0]/255.0f,((r[1]>>4)-(r[1]&15))/15.0f,ch*8,7,clock/384.0*(r[2]|(r[3]<<8))/2048); } return; }
    if(type == DEVID_YMW258) { for(unsigned ch=0; ch<28; ++ch) { auto r=&regs[ch*8]; int octave=(r[3]>>4); if(octave&8)octave-=16;
        auto& v=emit(ch,2,r[4]&128,0,1-(r[5]>>1)/127.0f,0,ch*8,8,clock/180.0*(1024+((r[3]&15)<<6)+(r[2]>>2))/1024*std::exp2(octave-1));
        std::snprintf(v.instrument,sizeof(v.instrument),"Sample %03X",r[1]|((r[2]&1)<<8)); } return; }
    if(type == DEVID_K054539) { for(unsigned ch=0; ch<8; ++ch) { unsigned b=ch*32, pitch=regs[b]|(regs[b+1]<<8)|(regs[b+2]<<16);
        emit(ch,2,keys[ch],0,std::pow(10.0f,-float(regs[b+3])*0.375f/20),0,b,16,clock/384.0*pitch/65536); } return; }
    if(type == DEVID_K053260) { for(unsigned ch=0; ch<4; ++ch) { unsigned b=8+ch*8,p=regs[b]|((regs[b+1]&15)<<8);
        emit(ch,2,regs[0x28]&(1<<ch),0,(regs[b+7]&127)/127.0f,0,b,8,clock/(32.0*(4096-p))); } return; }
    if(type == DEVID_C140 || type == DEVID_C219) { for(unsigned ch=0; ch<(type==DEVID_C219?16u:24u); ++ch) { unsigned b=ch*16;
        emit(ch,2,regs[b+5]&0x80,0,std::max(regs[b],regs[b+1])/255.0f,(int(regs[b])-int(regs[b+1]))/255.0f,b,16,clock/288.0*((regs[b+2]<<8)|regs[b+3])/32768); } return; }
    if(type == DEVID_C352) { for(unsigned ch=0; ch<32; ++ch) { unsigned b=ch*8;
        emit(ch,(regs[b+3]&16)?1:2,keys[ch],0,std::max(regs[b]&255,regs[b]>>8)/255.0f,0,b,8,original.sampleRate*regs[b+2]/65536.0); } return; }
    if(type == DEVID_ES5503) { for(unsigned ch=0; ch<32; ++ch)
        emit(ch,2,!(regs[0xa0+ch]&1),0,regs[0x40+ch]/255.0f,0,0xa0+ch,1,original.sampleRate*(regs[ch]|(regs[0x20+ch]<<8))/512.0); return; }
    if(type == DEVID_X1_010) { for(unsigned ch=0; ch<16; ++ch) { unsigned b=ch*8; bool wave=regs[b]&2; unsigned pitch=regs[b+2]|(regs[b+3]<<8);
        emit(ch,wave?0:2,regs[b]&1,wave?clock/512.0*pitch/131072.0:0,wave?1:std::max(regs[b+1]&15,regs[b+1]>>4)/15.0f,0,b,8,wave?0:clock/8192.0*regs[b+2]); } return; }
    if(type == DEVID_QSOUND) {
        for(unsigned ch=0;ch<16;++ch) { unsigned b=ch*8;
            emit(ch,2,regs[b+6]!=0,0,regs[b+6]/32768.0f,0,b,8,clock/166.0*regs[b+2]/4096); }
        for(unsigned ch=0;ch<3;++ch) emit(16+ch,2,regs[0xd6+ch]!=0,0,regs[0xcc+ch*4]/32768.0f,0,0xca+ch*4,4);
        return;
    }
    // Some programmable sample/DSP devices don't expose a universal voice
    // interpretation through their register API. Preserve their actual writes
    // explicitly, rather than labelling guessed pitches or activity as notes.
    auto& v=emit(0,4,false,0,0,0,0,0);
    std::snprintf(v.name,sizeof(v.name),"%s registers",name.c_str());
    int n=std::snprintf(v.details,sizeof(v.details),"Voice interpretation=not available | Writes=%u | Last write=%04X:%04X | Clock=%u Hz",writes,last_address,last_value,clock);
    for(unsigned a=0;a<regs.size() && n<int(sizeof(v.details))-20;++a) if(regs[a]) n+=std::snprintf(v.details+n,sizeof(v.details)-n," | R%03X=%04X",a,regs[a]);
}
} // namespace KogVgmInspection
