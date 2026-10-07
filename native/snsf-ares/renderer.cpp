// Copyright (c) 2026 Kog contributors
// SPDX-License-Identifier: GPL-3.0-or-later
#include "snsf.hpp"
#include "../embedded_stream.h"
#include <array>
#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <cstring>
#ifndef _WIN32
#include <poll.h>
#include <unistd.h>
#endif
#include <sfc/sfc.hpp>
#include "../ares-snsf/analyzer.hpp"
#include "../inspection_snes.h"
#include "../ares-snsf/boards.hpp"
#undef register
#include "../ares-snsf/reset.hpp"

namespace kog::snsf {
namespace {
constexpr uint32_t sampleRate = 32000;
// Standard 64-byte SPC700 IPL, already used in Kog's SFM backend. This hardware
// firmware is distinct from the ISC emulator source; no enhancement-chip ROMs
// are distributed by this adapter.
constexpr uint8_t ipl[] = {
  0xcd,0xef,0xbd,0xe8,0x00,0xc6,0x1d,0xd0,0xfc,0x8f,0xaa,0xf4,0x8f,0xbb,0xf5,0x78,
  0xcc,0xf4,0xd0,0xfb,0x2f,0x19,0xeb,0xf4,0xd0,0xfc,0x7e,0xf4,0xd0,0x0b,0xe4,0xf5,
  0xcb,0xf4,0xd7,0x00,0xfc,0xd0,0xf3,0xab,0x01,0x10,0xef,0x7e,0xf4,0x10,0xeb,0xba,
  0xf6,0xda,0x00,0xba,0xf4,0xc4,0xf4,0xdd,0x5d,0xd0,0xdb,0x1f,0x00,0x00,0xc0,0xff
};
std::string get(const Image& image, const char* key) {
    auto it = image.tags.find(key); return it == image.tags.end() ? "" : it->second;
}
uint32_t duration(const std::string& value, uint32_t fallback) {
    constexpr uint64_t maxMilliseconds = 86400000;
    if(value.empty()) {
        if(fallback > maxMilliseconds) throw std::runtime_error("SNSF duration exceeds 24 hours");
        return fallback;
    }
    double seconds = 0;
    size_t begin = 0;
    do {
        size_t end = value.find(':', begin);
        auto part = value.substr(begin, end == std::string::npos ? end : end - begin);
        size_t used = 0;
        double number = std::stod(part, &used);
        if(used != part.size() || !std::isfinite(number) || number < 0)
            throw std::runtime_error("Invalid SNSF duration tag");
        seconds = seconds * 60 + number;
        if(seconds > maxMilliseconds / 1000) throw std::runtime_error("SNSF duration exceeds 24 hours");
        if(end == std::string::npos) break;
        begin = end + 1;
    } while(true);
    return uint32_t(std::llround(seconds * 1000));
}
void bytes(FILE* out, const void* p, size_t count) {
    if(std::fwrite(p, 1, count, out) != count) throw std::runtime_error("SNSF PCM stream closed");
}
void integer(FILE* out, uint64_t value, size_t count) {
    uint8_t data[8];
    for(size_t n = 0; n < count; ++n) data[n] = value >> (8 * n);
    bytes(out, data, count);
}
struct Console final : ares::Platform {
    ares::Node::System node;
    ares::VFS::Pak systemPak = std::make_shared<nall::vfs::directory>();
    ares::VFS::Pak cartPak = std::make_shared<nall::vfs::directory>();
    std::vector<uint8_t> pcm;
    uint64_t position = 0, start, total;
    Console(Image& image, uint64_t startFrame, uint64_t totalFrames) : start(startFrame), total(totalFrames) {
        if(ares::platform) throw std::runtime_error("SNSF renderer reentered on its worker thread");
        resetSnsfMachine();
        pcm.reserve(4096);
        systemPak->append("ipl.rom", std::span<const uint8_t>(ipl));
        systemPak->append("boards.bml", std::span<const uint8_t>(reinterpret_cast<const uint8_t*>(snsfBoards), sizeof(snsfBoards)-1));
        SnsfCartridgeAnalysis analyzer;
        analyzer.rom = image.rom;
        const std::array<uint32_t, 4> headers{0x7fb0,0xffb0,0x407fb0,0x40ffb0};
        uint32_t best = 0;
        analyzer.headerAddress = headers[0];
        for(auto at : headers) {
            auto score = analyzer.scoreHeader(at);
            if(score && at >= 0x400000) score += 4;
            if(score > best) { best = score; analyzer.headerAddress = at; }
        }
        const auto memory = get(image, "_memory");
        if(memory == "LoROM") analyzer.headerAddress = image.rom.size() > 0x400000 ? 0x407fb0 : 0x7fb0;
        else if(memory == "HiROM") analyzer.headerAddress = image.rom.size() > 0x400000 ? 0x40ffb0 : 0xffb0;
        else if(!memory.empty()) throw std::runtime_error("Invalid SNSF _memory tag");
        if(analyzer.headerAddress + 0x50 > image.rom.size()) throw std::runtime_error("SNSF forced mapper header is outside ROM");
        // Respect a forced map while retaining the chip/type description.
        if(!memory.empty()) analyzer.rom[analyzer.headerAddress + 0x25] = memory == "LoROM" ? 0x20 : 0x21;
        auto board = analyzer.board();
        for(auto prefix : {"ARM-", "NEC-", "EXNEC-", "HITACHI-", "GB-", "BS-", "ST-"})
            if(board.beginsWith(prefix)) throw std::runtime_error("SNSF cartridge needs external firmware or a secondary cartridge");
        auto region = analyzer.videoRegion();
        auto video = get(image, "_video");
        if(!video.empty()) {
            if(video != "NTSC" && video != "PAL") throw std::runtime_error("Invalid SNSF _video tag");
            region = video.c_str();
        }
        auto definition = nall::BML::unserialize(snsfBoards);
        bool found = false;
        for(auto leaf : definition.find("board")) if(leaf.text() == board) found = true;
        if(!found) throw std::runtime_error("Unsupported SNSF cartridge board: " + std::string(board.data()));
        cartPak->setAttribute("board", board);
        cartPak->setAttribute("region", region);
        cartPak->setAttribute("title", "SNSF");
        const size_t program = analyzer.programRomSize(), data = analyzer.dataRomSize();
        if(program > image.rom.size() || data > image.rom.size()-program)
            throw std::runtime_error("SNSF cartridge ROM partition exceeds file");
        cartPak->append("program.rom", std::span<const uint8_t>(image.rom.data(), program));
        if(data) cartPak->append("data.rom", std::span<const uint8_t>(image.rom.data()+program, data));
        if(analyzer.expansionRomSize()) {
            const auto extra = analyzer.expansionRomSize();
            if(extra > image.rom.size()-program-data) throw std::runtime_error("SNSF expansion ROM exceeds file");
            cartPak->append("expansion.rom", std::span<const uint8_t>(image.rom.data()+program+data, extra));
        }
        const size_t ram = std::max(analyzer.ramSize(), analyzer.expansionRamSize());
        if(ram) {
            std::vector<uint8_t> save(ram, 255);
            std::copy_n(image.sram.begin(), std::min(ram, image.sram.size()), save.begin());
            cartPak->append("save.ram", std::span<const uint8_t>(save));
        }
        ares::platform = this;
        try {
            ares::SuperFamicom::option("Pixel Accuracy", "true");
            ares::SuperFamicom::option("Deterministic Entropy", "true");
            if(!ares::SuperFamicom::load(node, region == "PAL" ? "[Nintendo] Super Famicom (PAL)" : "[Nintendo] Super Famicom (NTSC)"))
                throw std::runtime_error("Could not initialize ares SNSF core");
            auto& slot = ares::SuperFamicom::cartridgeSlot;
            slot.port->allocate(); slot.port->connect();
            ares::SuperFamicom::system.power(false);
            ares::SuperFamicom::dsp.stream->setResamplerFrequency(sampleRate);
        } catch(...) { if(node) ares::SuperFamicom::system.unload(); ares::platform = nullptr; throw; }
    }
    ~Console() { ares::SuperFamicom::system.unload(); ares::platform = nullptr; }
    ares::VFS::Pak pak(ares::Node::Object object) override {
        return object->name() == "Super Famicom" ? systemPak : cartPak;
    }
    void audio(ares::Node::Audio::Stream stream) override {
        double values[2];
        while(stream->pending()) {
            stream->read(values);
            if(position >= start && position < total) {
                for(auto value : values) {
                    const auto sample = uint16_t(int16_t(std::clamp(std::lrint(value * 32768), -32768L, 32767L)));
                    pcm.push_back(uint8_t(sample)); pcm.push_back(uint8_t(sample >> 8));
                }
            }
            ++position;
            if(position % (sampleRate / 200) == 0 && kog_inspection_enabled()) {
                uint8_t regs[128];
                for(unsigned i = 0; i < 128; ++i) regs[i] = ares::SuperFamicom::dsp.registers[i];
                KogVoice data[8]; KogVoices voices{data, 8};
                kog_inspect_snes(regs, voices);
                kog_inspection_publish(double(position) / sampleRate, data, voices.count);
            }
        }
    }
};
}
void render(Image& image, FILE* out, uint64_t start, uint32_t length, uint32_t fade) {
    const auto taggedLength = get(image, "length");
    uint32_t durationMs = duration(taggedLength, length);
    const bool explicitLength = !taggedLength.empty() && durationMs != 0;
    if(!durationMs) durationMs = duration("", length);
    uint64_t mainFrames = uint64_t(durationMs) * sampleRate / 1000;
    uint64_t total = mainFrames + uint64_t(duration(get(image, "fade"), explicitLength ? 0 : fade)) * sampleRate / 1000;
    Console console(image, start, total);
    bytes(out, "KOGPSF1\0", 8);
    integer(out, 1, 4); integer(out, 0x23, 4); integer(out, sampleRate, 4); integer(out, 2, 4);
    integer(out, total, 8); integer(out, mainFrames, 8);
    std::array<std::string,5> fields{get(image,"title"),get(image,"artist"),get(image,"game"),get(image,"genre"),get(image,"date")};
    if(fields[4].empty()) fields[4] = get(image,"year");
    size_t metadataSize = 0;
    for(auto& field : fields) metadataSize += field.size();
    if(metadataSize > 65536) throw std::runtime_error("SNSF metadata exceeds 64 KiB");
    for(auto& field : fields) integer(out, field.size(), 4);
    for(auto& field : fields) bytes(out, field.data(), field.size());
    if(std::fflush(out)) throw std::runtime_error("SNSF PCM stream closed");
    if(start >= total) return;
    while(console.position < total) {
        if(kog_embedded_stream_cancelled(out)) throw std::runtime_error("SNSF render cancelled");
#ifndef _WIN32
        pollfd writer{fileno(out), POLLOUT, 0};
        if(poll(&writer, 1, 0) > 0 && (writer.revents & (POLLERR | POLLHUP | POLLNVAL)))
            throw std::runtime_error("SNSF PCM stream cancelled");
#endif
        ares::SuperFamicom::system.run();
        bytes(out, console.pcm.data(), console.pcm.size());
        console.pcm.clear();
        if(std::fflush(out)) throw std::runtime_error("SNSF PCM stream closed");
    }
}
}
