// Copyright (c) 2026 Kog contributors
// SPDX-License-Identifier: GPL-3.0-or-later
// Format: https://snsf.caitsith2.net/snsf%20spec.txt (version 0.03).
#include "snsf.hpp"
#include "../psflib/psflib.h"
#include <zlib.h>
#include <algorithm>
#include <array>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <memory>
#include <stdexcept>
#include <limits>

namespace kog::snsf {
namespace {
constexpr size_t romLimit = 16 * 1024 * 1024;
constexpr size_t fileLimit = 32 * 1024 * 1024;
constexpr size_t budgetLimit = 256 * 1024 * 1024;
uint32_t word(const uint8_t* p) {
    return uint32_t(p[0]) | uint32_t(p[1]) << 8 | uint32_t(p[2]) << 16 | uint32_t(p[3]) << 24;
}
void require(bool ok, const char* reason) { if(!ok) throw std::runtime_error(reason); }
struct Input { std::vector<uint8_t> bytes; size_t pos = 0; };
struct Loader {
    Image image;
    std::filesystem::path rootDirectory;
    size_t budget = 0;
    unsigned opens = 0;
    bool hasBase = false;
    uint32_t base = 0;
    std::string error;
};
// Validate each actual opened byte sequence before psflib can allocate/decompress it.
// The same immutable buffer is then used by psflib, avoiding a validation/read race.
void* openFile(void* opaque, const char* path) noexcept {
    auto& loader = *static_cast<Loader*>(opaque);
    try {
        require(++loader.opens <= 128, "SNSF dependency count exceeds 128");
        // Resolve both parent components and symlinks before opening a dependency.
        // Compare path components, so a sibling named "music-extra" does not
        // satisfy containment in a directory named "music".
        const auto resolved = std::filesystem::canonical(std::filesystem::u8path(path));
        auto component = resolved.begin();
        for(const auto& rootComponent : loader.rootDirectory) {
            require(component != resolved.end() && *component == rootComponent,
                    "SNSF dependency escapes the root file directory");
            ++component;
        }
        require(component != resolved.end(), "SNSF dependency must name a file");
        std::ifstream stream(resolved, std::ios::binary | std::ios::ate);
        require(bool(stream), "Cannot open SNSF dependency");
        auto size = stream.tellg();
        require(size >= 16 && size <= fileLimit, "SNSF file size is outside the supported bounds");
        require(size_t(size) <= budgetLimit - loader.budget, "SNSF dependency byte budget exceeded");
        loader.budget += size_t(size);
        auto input = std::make_unique<Input>();
        input->bytes.resize(size_t(size));
        stream.seekg(0);
        require(bool(stream.read(reinterpret_cast<char*>(input->bytes.data()), size)), "Cannot read SNSF dependency");
        const auto& b = input->bytes;
        require(std::memcmp(b.data(), "PSF\x23", 4) == 0, "Expected SNSF version 0x23");
        size_t reserved = word(b.data() + 4), compressed = word(b.data() + 8);
        require(reserved <= 1024 * 1024 && reserved <= b.size() - 16, "SNSF reserved section exceeds bounds");
        require(compressed <= b.size() - 16 - reserved, "SNSF compressed section exceeds file bounds");
        require(b.size() - 16 - reserved - compressed <= 65536, "SNSF tags exceed 64 KiB");
        if(compressed) {
            const auto* payload = b.data() + 16 + reserved;
            require(crc32(0, payload, compressed) == word(b.data() + 12), "SNSF program CRC mismatch");
            z_stream z{};
            require(inflateInit(&z) == Z_OK, "Cannot initialize SNSF decompression");
            struct End { z_stream* z; ~End(){inflateEnd(z);} } end{&z};
            z.next_in = const_cast<Bytef*>(payload); z.avail_in = compressed;
            std::array<uint8_t, 32768> scratch;
            int result;
            do {
                z.next_out = scratch.data(); z.avail_out = scratch.size();
                result = inflate(&z, Z_NO_FLUSH);
                require(z.total_out <= romLimit + 8, "SNSF decompressed program exceeds 16 MiB");
                require(result == Z_OK || result == Z_STREAM_END, "Invalid SNSF compressed program");
            } while(result != Z_STREAM_END);
            require(z.avail_in == 0, "Trailing SNSF compressed program bytes");
            require(z.total_out <= budgetLimit - loader.budget, "SNSF decompression byte budget exceeded");
            loader.budget += z.total_out;
        }
        return input.release();
    } catch(const std::exception& e) { loader.error = e.what(); return nullptr; }
}
size_t readFile(void* buffer, size_t size, size_t count, void* opaque) {
    auto& in = *static_cast<Input*>(opaque);
    if(!size) return 0;
    auto available = (in.bytes.size() - in.pos) / size;
    auto items = std::min(count, available);
    std::memcpy(buffer, in.bytes.data() + in.pos, items * size);
    in.pos += items * size;
    return items;
}
int seekFile(void* opaque, int64_t offset, int whence) {
    auto& in = *static_cast<Input*>(opaque);
    int64_t base = whence == SEEK_SET ? 0 : whence == SEEK_CUR ? in.pos : whence == SEEK_END ? in.bytes.size() : -1;
    if(base < 0 || offset < -base || offset > int64_t(in.bytes.size()) - base) return -1;
    in.pos = size_t(base + offset); return 0;
}
int closeFile(void* in) { delete static_cast<Input*>(in); return 0; }
long tellFile(void* in) { return static_cast<Input*>(in)->pos; }
int section(void* opaque, const uint8_t* program, size_t programBytes,
            const uint8_t* reserved, size_t reservedBytes) noexcept {
    auto& l = *static_cast<Loader*>(opaque);
    try {
        if(programBytes) {
            require(programBytes >= 8, "Truncated SNSF ROM record");
            uint64_t destination = word(program);
            const size_t count = word(program + 4);
            if(l.hasBase) destination += l.base;
            else { l.base = destination; l.hasBase = true; }
            require(destination <= UINT32_MAX, "SNSF relative ROM address overflow");
            destination &= 0x1fffffff;
            require(count <= programBytes - 8, "Truncated SNSF ROM payload");
            require(destination <= romLimit && count <= romLimit - destination, "SNSF ROM address exceeds 16 MiB");
            l.image.rom.resize(std::max(l.image.rom.size(), size_t(destination) + count));
            std::copy_n(program + 8, count, l.image.rom.begin() + destination);
        }
        size_t cursor = 0;
        while(cursor < reservedBytes) {
            require(reservedBytes - cursor >= 8, "Truncated SNSF reserved record");
            uint32_t type = word(reserved + cursor), count = word(reserved + cursor + 4);
            cursor += 8;
            require(count <= reservedBytes - cursor, "Truncated SNSF reserved payload");
            if(type == UINT32_MAX) { require(count == 0, "Invalid SNSF reserved terminator"); break; }
            require(type == 0, "SNSF save-state records are unsupported");
            require(count >= 4, "Truncated SNSF SRAM address");
            size_t at = word(reserved + cursor), bytes = count - 4;
            require(at <= l.image.sram.size() && bytes <= l.image.sram.size() - at, "SNSF SRAM exceeds 128 KiB");
            std::copy_n(reserved + cursor + 4, bytes, l.image.sram.begin() + at);
            std::fill_n(l.image.sramWritten.begin() + at, bytes, true);
            cursor += count;
        }
        return 0;
    } catch(const std::exception& e) { l.error = e.what(); return -1; }
}
int tag(void* opaque, const char* name, const char* value) noexcept {
    auto& l = *static_cast<Loader*>(opaque);
    try {
        require(std::strlen(name) <= 128 && std::strlen(value) <= 16384 && l.image.tags.size() < 256, "SNSF metadata exceeds bounds");
        std::string key(name);
        for(auto& c : key) if(c >= 'A' && c <= 'Z') c += 'a' - 'A';
        // psflib sends the root tags first, then nested libraries.
        l.image.tags.try_emplace(key, value);
        return 0;
    } catch(const std::exception& e) { l.error = e.what(); return -1; }
}
}
Image load(const char* path) {
    Loader l;
    const auto root = std::filesystem::canonical(std::filesystem::u8path(path));
    l.rootDirectory = root.parent_path();
    const auto rootUtf8 = root.generic_u8string();
    const std::string rootName(reinterpret_cast<const char*>(rootUtf8.data()), rootUtf8.size());
    psf_file_callbacks callbacks{"/\\", &l, openFile, readFile, seekFile, closeFile, tellFile};
    if(psf_load(rootName.c_str(), &callbacks, 0x23, section, &l, tag, &l, 1, nullptr, nullptr) < 0)
        throw std::runtime_error(l.error.empty() ? "SNSF dependency loading failed" : l.error);
    require(l.image.rom.size() >= 32768, "SNSF ROM is smaller than 32 KiB");
    if(auto entry = l.image.tags.find("_sramfill"); entry != l.image.tags.end()) {
        size_t parsed;
        auto fill = std::stoul(entry->second, &parsed, 0);
        require(parsed == entry->second.size() && fill <= 255, "Invalid SNSF SRAM fill value");
        for(size_t n = 0; n < l.image.sram.size(); ++n)
            if(!l.image.sramWritten[n]) l.image.sram[n] = fill;
    }
    return std::move(l.image);
}
}
