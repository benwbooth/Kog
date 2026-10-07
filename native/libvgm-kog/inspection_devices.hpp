#pragma once
#include "../inspection.h"
#include "../libvgm/emu/SoundEmu.h"
#include "../libvgm/emu/SoundDevs.h"
#include <array>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

namespace KogVgmInspection {
struct Device;
using Devices = std::vector<Device*>;
inline thread_local Devices* creating = nullptr;
struct CreationScope {
    Devices* previous;
    explicit CreationScope(Devices& devices) : previous(creating) { creating = &devices; }
    ~CreationScope() { creating = previous; }
};

// A forwarding device records the writes sent to the very same sound core.
// No replay, second emulator, muting, or audio analysis is used for note data.
struct Device {
    DEV_DATA data{this};
    DEV_INFO original{};
    DEV_DEF definition{};
    std::array<DEVDEF_RWFUNC, 65> functions{};
    std::array<void*, 64> original_functions{};
    Devices* owner = nullptr;
    unsigned type = 0, clock = 0, flags = 0, id = 0;
    std::string name;
    std::vector<std::string> channel_names;
    std::array<uint16_t, 65536> regs{};
    std::array<uint8_t, 256> latches{};
    std::array<uint8_t, 64> keys{};
    std::array<uint16_t, 8> sn{};
    uint32_t writes = 0, last_address = 0, last_value = 0;
    unsigned bank = 0, selected_channel = 0, sn_latch = 0;
    bool linked = false;

    void clear() {
        regs.fill(0); latches.fill(0); keys.fill(0); sn.fill(0);
        regs[0xff] = type == DEVID_SN76496 ? 0xff : 0;
        for(unsigned i = 1; i < 8; i += 2) sn[i] = 15;
        bank = selected_channel = sn_latch = writes = last_address = last_value = 0;
    }
    void write(unsigned address, unsigned value, unsigned width, unsigned rw_index);
    void snapshot(KogVoices& voices) const;

    static Device& self(void* p) { return *static_cast<Device*>(static_cast<DEV_DATA*>(p)->chipInf); }
    static void stop(void* p) {
        auto& d = self(p);
        d.original.devDef->Stop(d.original.dataPtr);
        if(d.owner) d.owner->erase(std::remove(d.owner->begin(), d.owner->end(), &d), d.owner->end());
        delete &d;
    }
    static void reset(void* p) { auto& d = self(p); d.clear(); d.original.devDef->Reset(d.original.dataPtr); }
    static void update(void* p, UINT32 n, DEV_SMPL** out) { auto& d = self(p); d.original.devDef->Update(d.original.dataPtr, n, out); }
    static void option(void* p, UINT32 value) { auto& d = self(p); d.original.devDef->SetOptionBits(d.original.dataPtr, value); }
    static void mute(void* p, UINT32 value) { auto& d = self(p); d.original.devDef->SetMuteMask(d.original.dataPtr, value); }
    static void pan(void* p, const INT16* value) { auto& d = self(p); d.original.devDef->SetPanning(d.original.dataPtr, value); }
    static void sample_rate(void* p, DEVCB_SRATE_CHG cb, void* context) { auto& d = self(p); d.original.devDef->SetSRateChgCB(d.original.dataPtr, cb, context); }
    static void log(void* p, DEVCB_LOG cb, void* context) { auto& d = self(p); d.original.devDef->SetLogCB(d.original.dataPtr, cb, context); }
    static UINT8 link(void* p, UINT8 id, const DEV_INFO* other) {
        auto& d = self(p);
        if(other && other->devDef->Stop == stop) {
            auto& linked = self(other->dataPtr); linked.linked = true;
            return d.original.devDef->LinkDevice(d.original.dataPtr, id, &linked.original);
        }
        return d.original.devDef->LinkDevice(d.original.dataPtr, id, other);
    }
    template<size_t I, class A, class D> static void write_reg(void* p, A a, D v) {
        auto& d = self(p);
        reinterpret_cast<void(*)(void*, A, D)>(d.original_functions[I])(d.original.dataPtr, a, v);
        if((d.functions[I].funcType & 0xf0) == RWF_REGISTER) d.write(a, v, sizeof(A) * 8 + sizeof(D), I);
    }
    template<size_t I, class A, class D> static D read_reg(void* p, A a) {
        auto& d = self(p);
        return reinterpret_cast<D(*)(void*, A)>(d.original_functions[I])(d.original.dataPtr, a);
    }
    template<size_t I, class D> static void write_value(void* p, D value) {
        auto& d = self(p);
        reinterpret_cast<void(*)(void*, D)>(d.original_functions[I])(d.original.dataPtr, value);
        if((d.functions[I].funcType & 0xfe) == RWF_CLOCK) d.clock = static_cast<unsigned>(value);
    }
    template<size_t I> static UINT32 read_value(void* p) {
        auto& d = self(p);
        return reinterpret_cast<DEVFUNC_READ_CLOCK>(d.original_functions[I])(d.original.dataPtr);
    }
    template<size_t I> static void block(void* p, UINT32 offset, UINT32 length, const UINT8* data) {
        auto& d = self(p);
        reinterpret_cast<DEVFUNC_WRITE_BLOCK>(d.original_functions[I])(d.original.dataPtr, offset, length, data);
    }
    template<size_t I> static void volume_lr(void* p, INT32 left, INT32 right) {
        auto& d = self(p);
        reinterpret_cast<DEVFUNC_WRITE_VOL_LR>(d.original_functions[I])(d.original.dataPtr, left, right);
    }
    template<size_t I> static void pans(void* p, const INT16* values) {
        auto& d = self(p);
        reinterpret_cast<DEVFUNC_PANALL>(d.original_functions[I])(d.original.dataPtr, values);
    }
    template<size_t I> void wrap() {
        auto& f = functions[I];
        if(!f.funcPtr) return;
        original_functions[I] = f.funcPtr;
        const bool read = f.funcType & RWF_READ;
        if((f.funcType & 0xf0) < 0x80) {
            switch(f.rwType) {
            case DEVRW_A8D8: f.funcPtr = read ? reinterpret_cast<void*>(read_reg<I, UINT8, UINT8>) : reinterpret_cast<void*>(write_reg<I, UINT8, UINT8>); break;
            case DEVRW_A8D16: f.funcPtr = read ? reinterpret_cast<void*>(read_reg<I, UINT8, UINT16>) : reinterpret_cast<void*>(write_reg<I, UINT8, UINT16>); break;
            case DEVRW_A16D8: f.funcPtr = read ? reinterpret_cast<void*>(read_reg<I, UINT16, UINT8>) : reinterpret_cast<void*>(write_reg<I, UINT16, UINT8>); break;
            case DEVRW_A16D16: f.funcPtr = read ? reinterpret_cast<void*>(read_reg<I, UINT16, UINT16>) : reinterpret_cast<void*>(write_reg<I, UINT16, UINT16>); break;
            case DEVRW_MEMSIZE: f.funcPtr = reinterpret_cast<void*>(write_value<I, UINT32>); break;
            case DEVRW_ALL: f.funcPtr = reinterpret_cast<void*>(write_value<I, UINT32>); break; // AY stereo mask
            case DEVRW_BLOCK: f.funcPtr = reinterpret_cast<void*>(block<I>); break;
            default: throw std::runtime_error("libvgm inspection encountered an unknown device function");
            }
        } else if(read) f.funcPtr = reinterpret_cast<void*>(read_value<I>);
        else if((f.funcType & 0xfe) == RWF_CHN_PAN && f.rwType == DEVRW_ALL) f.funcPtr = reinterpret_cast<void*>(pans<I>);
        else if((f.funcType & 0xfe) == RWF_VOLUME_LR) f.funcPtr = reinterpret_cast<void*>(volume_lr<I>);
        else if((f.funcType & 0xfe) == RWF_VOLUME) f.funcPtr = reinterpret_cast<void*>(write_value<I, INT32>);
        else f.funcPtr = reinterpret_cast<void*>(write_value<I, UINT32>);
    }
    template<size_t... I> void wrap_all(std::index_sequence<I...>) { (wrap<I>(), ...); }

    template<size_t Type> static UINT8 start(const DEV_GEN_CFG* config, DEV_INFO* result) {
        return start_type(static_cast<DEV_ID>(Type), config, result);
    }
    static UINT8 start_type(DEV_ID Type, const DEV_GEN_CFG* config, DEV_INFO* result) {
        auto d = std::make_unique<Device>();
        const auto error = SndEmu_Start(static_cast<DEV_ID>(Type), config, &d->original);
        if(error) return error;
        d->owner = creating;
        d->id = creating ? static_cast<unsigned>(creating->size()) : 0;
        d->type = Type; d->clock = config->clock; d->flags = config->flags;
        const auto* declaration = d->original.devDecl;
        d->name = declaration->name(config);
        const auto count = declaration->channelCount(config);
        const auto* names = declaration->channelNames ? declaration->channelNames(config) : nullptr;
        for(unsigned i = 0; i < count; ++i) d->channel_names.push_back(names && names[i] ? names[i] : std::to_string(i + 1));
        d->definition = *d->original.devDef;
        auto& f = d->definition;
        f.Stop = stop; f.Reset = reset; f.Update = update;
        if(f.SetOptionBits) f.SetOptionBits = option;
        if(f.SetMuteMask) f.SetMuteMask = mute;
        if(f.SetPanning) f.SetPanning = pan;
        if(f.SetSRateChgCB) f.SetSRateChgCB = sample_rate;
        if(f.SetLogCB) f.SetLogCB = log;
        if(f.LinkDevice) f.LinkDevice = link;
        size_t n = 0;
        for(; d->original.devDef->rwFuncs[n].funcPtr && n < 64; ++n) d->functions[n] = d->original.devDef->rwFuncs[n];
        if(n == 64 && d->original.devDef->rwFuncs[n].funcPtr) {
            d->original.devDef->Stop(d->original.dataPtr); return EERR_INIT_ERR;
        }
        d->wrap_all(std::make_index_sequence<64>{});
        f.rwFuncs = d->functions.data(); d->clear();
        *result = d->original; result->devDef = &f; result->dataPtr = &d->data;
        if(creating) creating->push_back(d.get());
        d.release();
        return 0;
    }
};

// Preserve every core ID, configuration callback and channel name. Selecting a
// nondefault emulator still forwards to SndEmu's original built-in core list.
struct Declaration {
    DEV_ID type;
    DEVDECLFUNC_NAME name;
    DEVDECLFUNC_CHNCOUNT count;
    DEVDECLFUNC_CHNNAMES names;
    DEVDECLFUNC_LINKIDS links;
    const DEV_DEF* cores[17]{};
    std::array<DEV_DEF, 16> wrapped{};
};
static_assert(offsetof(Declaration, cores) == offsetof(DEV_DECL, cores));
template<size_t... I> constexpr auto starts(std::index_sequence<I...>) {
    return std::array<DEVFUNC_START, sizeof...(I)>{Device::start<I>...};
}
struct Declarations {
    std::vector<std::unique_ptr<Declaration>> storage;
    std::vector<const DEV_DECL*> list;
    Declarations() {
        const auto callbacks = starts(std::make_index_sequence<256>{});
        for(auto ptr = sndEmu_Devices; *ptr; ++ptr) {
            const auto& source = **ptr;
            auto d = std::make_unique<Declaration>();
            d->type = source.deviceID; d->name = source.name; d->count = source.channelCount;
            d->names = source.channelNames; d->links = source.linkDevIDs;
            for(size_t i = 0; source.cores[i] && i < 16; ++i) {
                d->wrapped[i] = *source.cores[i];
                d->wrapped[i].Start = callbacks[source.deviceID];
                // SndEmu_StartCore calls Reset through the declaration,
                // before it returns the per-instance devDef to the player.
                d->wrapped[i].Reset = Device::reset;
                d->cores[i] = &d->wrapped[i];
            }
            list.push_back(reinterpret_cast<const DEV_DECL*>(d.get())); storage.push_back(std::move(d));
        }
        list.push_back(nullptr);
    }
};
inline const DEV_DECL** declarations() { static Declarations devices; return devices.list.data(); }
} // namespace KogVgmInspection
