/*
 * Kog PSF2 helper process.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Play! is reused as a separate process. Kog validates the legacy container
 * parsers' inputs here before constructing Play!'s PSF2 filesystem and IOP.
 */

#include <algorithm>
#include <array>
#include <cerrno>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <limits>
#include <map>
#include <memory>
#include <set>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#endif

#include <zstd_zlibwrapper.h>

#include "Iop_PsfSubSystem.h"
#include "PsfBase.h"
#include "Ps2Const.h"
#include "StdStreamUtils.h"
#include "app_shared/AppConfig.h"
#include "iop/IopBios.h"
#include "ps2/Ps2_PsfDevice.h"

fs::path CAppConfig::GetBasePath() const
{
    return fs::temp_directory_path() / "Kog PSF2 Helper";
}

namespace
{
constexpr uint64_t MAX_FILE_BYTES = 256ULL * 1024ULL * 1024ULL;
constexpr uint64_t MAX_RESERVED_BYTES = 128ULL * 1024ULL * 1024ULL;
constexpr uint64_t MAX_PROGRAM_BYTES = 32ULL * 1024ULL * 1024ULL;
constexpr uint64_t MAX_FS_FILE_BYTES = 64ULL * 1024ULL * 1024ULL;
constexpr uint64_t MAX_FS_TOTAL_BYTES = 256ULL * 1024ULL * 1024ULL;
constexpr uint32_t MAX_COMPRESSED_BLOCK_BYTES = 1024U * 1024U;
constexpr uint32_t MAX_FS_ENTRIES = 65536U;
constexpr unsigned int MAX_FS_DEPTH = 32U;
constexpr unsigned int MAX_LIBRARY_DEPTH = 16U;
constexpr uint32_t MAX_DURATION_MILLISECONDS = 24U * 60U * 60U * 1000U;
constexpr uint32_t SAMPLE_RATE = 44100U;
constexpr uint32_t CHANNELS = 2U;
constexpr std::array<uint8_t, 8> HELPER_MAGIC = {'K', 'O', 'G', 'P', 'S', 'F', '1', 0};

using Bytes = std::vector<uint8_t>;
using Tags = std::map<std::string, std::string>;

struct ParsedPsf
{
    Tags tags;
    bool hasRootIrx = false;
};

struct ValidationContext
{
    uint64_t totalFsBytes = 0;
    uint32_t totalFsEntries = 0;
    bool hasRootIrx = false;
    std::set<std::string> activeLibraries;
    std::set<std::string> loadedLibraries;
    std::vector<fs::path> loadOrder;
    Tags fallbackTags;
};

uint16_t readU16(const uint8_t* bytes)
{
    return static_cast<uint16_t>(bytes[0]) |
           static_cast<uint16_t>(static_cast<uint16_t>(bytes[1]) << 8U);
}

uint32_t readU32(const uint8_t* bytes)
{
    return static_cast<uint32_t>(bytes[0]) |
           (static_cast<uint32_t>(bytes[1]) << 8U) |
           (static_cast<uint32_t>(bytes[2]) << 16U) |
           (static_cast<uint32_t>(bytes[3]) << 24U);
}

bool checkedRange(uint64_t offset, uint64_t size, uint64_t length)
{
    return offset <= length && size <= length - offset;
}

std::string lowerAscii(std::string text)
{
    for(char& character : text)
    {
        if(character >= 'A' && character <= 'Z')
        {
            character = static_cast<char>(character - 'A' + 'a');
        }
    }
    return text;
}

std::string displayPath(const fs::path& path)
{
#ifdef _WIN32
    const auto wide = path.wstring();
    if(wide.empty()) return {};
    const int length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide.c_str(), -1,
                                            nullptr, 0, nullptr, nullptr);
    if(length <= 0) return "<invalid path>";
    std::string result(static_cast<size_t>(length), '\0');
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide.c_str(), -1, result.data(), length,
                        nullptr, nullptr);
    result.pop_back();
    return result;
#else
    return path.string();
#endif
}

Bytes readFile(const fs::path& path)
{
    std::ifstream stream(path, std::ios::binary | std::ios::ate);
    if(!stream)
    {
        throw std::runtime_error("cannot open PSF2 dependency " + displayPath(path));
    }
    const auto end = stream.tellg();
    if(end < 0 || static_cast<uint64_t>(end) > MAX_FILE_BYTES)
    {
        throw std::runtime_error("PSF2 file has an unsupported size: " + displayPath(path));
    }
    Bytes bytes(static_cast<size_t>(end));
    stream.seekg(0);
    if(!bytes.empty() && !stream.read(reinterpret_cast<char*>(bytes.data()), end))
    {
        throw std::runtime_error("short read while validating " + displayPath(path));
    }
    return bytes;
}

void validateCompressedProgram(const uint8_t* compressed, uint32_t compressedLength,
                               uint32_t expectedCrc, const fs::path& path)
{
    if(compressedLength == 0)
    {
        if(expectedCrc != 0)
        {
            throw std::runtime_error("PSF2 has a CRC without a program section: " +
                                     displayPath(path));
        }
        return;
    }
    if(static_cast<uint32_t>(crc32(0, compressed, compressedLength)) != expectedCrc)
    {
        throw std::runtime_error("PSF2 program CRC mismatch in " + displayPath(path));
    }

    z_stream inflater = {};
    inflater.next_in = const_cast<Bytef*>(reinterpret_cast<const Bytef*>(compressed));
    inflater.avail_in = compressedLength;
    if(inflateInit(&inflater) != Z_OK)
    {
        throw std::runtime_error("cannot initialize PSF2 program decompression");
    }
    std::array<uint8_t, 16384> output = {};
    uint64_t total = 0;
    int result = Z_OK;
    while(result == Z_OK)
    {
        inflater.next_out = output.data();
        inflater.avail_out = static_cast<uInt>(output.size());
        result = inflate(&inflater, Z_NO_FLUSH);
        total += output.size() - inflater.avail_out;
        if(total > MAX_PROGRAM_BYTES)
        {
            inflateEnd(&inflater);
            throw std::runtime_error("PSF2 program exceeds Kog's decompression limit");
        }
    }
    const bool valid = result == Z_STREAM_END && inflater.avail_in == 0;
    inflateEnd(&inflater);
    if(!valid)
    {
        throw std::runtime_error("invalid compressed PSF2 program in " + displayPath(path));
    }
}

Tags parseTags(const Bytes& bytes, size_t offset, const fs::path& path)
{
    Tags tags;
    if(offset == bytes.size()) return tags;
    if(!checkedRange(offset, 5, bytes.size()) ||
       std::memcmp(bytes.data() + offset, "[TAG]", 5) != 0)
    {
        return tags;
    }
    offset += 5;
    if(bytes.size() - offset > 64U * 1024U)
    {
        throw std::runtime_error("PSF2 tags exceed Kog's 64 KiB limit in " + displayPath(path));
    }

    unsigned int tagCount = 0;
    while(offset < bytes.size())
    {
        size_t lineEnd = offset;
        while(lineEnd < bytes.size() && bytes[lineEnd] != '\n') ++lineEnd;
        size_t valueEnd = lineEnd;
        if(valueEnd > offset && bytes[valueEnd - 1] == '\r') --valueEnd;
        if(valueEnd != offset)
        {
            if(++tagCount > 128U || valueEnd - offset > 1024U ||
               std::memchr(bytes.data() + offset, 0, valueEnd - offset) != nullptr)
            {
                throw std::runtime_error("invalid PSF2 tag data in " + displayPath(path));
            }
            const auto equal = std::find(bytes.begin() + static_cast<ptrdiff_t>(offset),
                                         bytes.begin() + static_cast<ptrdiff_t>(valueEnd),
                                         static_cast<uint8_t>('='));
            if(equal != bytes.begin() + static_cast<ptrdiff_t>(valueEnd))
            {
                const size_t equalOffset = static_cast<size_t>(equal - bytes.begin());
                std::string name(reinterpret_cast<const char*>(bytes.data() + offset),
                                 equalOffset - offset);
                std::string value(reinterpret_cast<const char*>(bytes.data() + equalOffset + 1),
                                  valueEnd - equalOffset - 1);
                name = lowerAscii(name);
                if(!name.empty()) tags[name] = value;
            }
        }
        offset = lineEnd < bytes.size() ? lineEnd + 1 : bytes.size();
    }
    return tags;
}

void validateIrx(const Bytes& irx, const fs::path& path)
{
    constexpr uint32_t PT_LOAD = 1;
    constexpr uint32_t SHT_NOBITS = 8;
    constexpr uint32_t IOPMOD_SECTION = 0x70000080;
    constexpr size_t ELF_HEADER_SIZE = 52;
    constexpr size_t PROGRAM_HEADER_SIZE = 32;
    constexpr size_t SECTION_HEADER_SIZE = 40;
    constexpr size_t IOPMOD_SIZE = 282;

    if(irx.size() < ELF_HEADER_SIZE || std::memcmp(irx.data(), "\x7f" "ELF", 4) != 0 ||
       irx[4] != 1 || irx[5] != 1 || readU16(irx.data() + 18) != 8)
    {
        throw std::runtime_error("psf2.irx is not a little-endian MIPS ELF in " +
                                 displayPath(path));
    }
    const uint32_t entryPoint = readU32(irx.data() + 24);
    const uint32_t programOffset = readU32(irx.data() + 28);
    const uint32_t sectionOffset = readU32(irx.data() + 32);
    const uint16_t programEntrySize = readU16(irx.data() + 42);
    const uint16_t programCount = readU16(irx.data() + 44);
    const uint16_t sectionEntrySize = readU16(irx.data() + 46);
    const uint16_t sectionCount = readU16(irx.data() + 48);
    if(programEntrySize != PROGRAM_HEADER_SIZE || programCount == 0 || programCount > 64 ||
       sectionEntrySize != SECTION_HEADER_SIZE || sectionCount == 0 || sectionCount > 256 ||
       !checkedRange(programOffset, static_cast<uint64_t>(programCount) * PROGRAM_HEADER_SIZE,
                     irx.size()) ||
       !checkedRange(sectionOffset, static_cast<uint64_t>(sectionCount) * SECTION_HEADER_SIZE,
                     irx.size()))
    {
        throw std::runtime_error("psf2.irx has invalid ELF tables in " + displayPath(path));
    }

    uint32_t loadCount = 0;
    uint32_t loadMemorySize = 0;
    for(uint16_t index = 0; index < programCount; ++index)
    {
        const uint8_t* header = irx.data() + programOffset + index * PROGRAM_HEADER_SIZE;
        if(readU32(header) != PT_LOAD) continue;
        ++loadCount;
        const uint32_t fileOffset = readU32(header + 4);
        const uint32_t fileSize = readU32(header + 16);
        loadMemorySize = readU32(header + 20);
        if(fileSize > loadMemorySize || loadMemorySize == 0 ||
           loadMemorySize > PS2::IOP_RAM_SIZE || !checkedRange(fileOffset, fileSize, irx.size()))
        {
            throw std::runtime_error("psf2.irx load segment exceeds emulated IOP RAM in " +
                                     displayPath(path));
        }
    }
    if(loadCount != 1 || entryPoint >= loadMemorySize)
    {
        throw std::runtime_error("psf2.irx must contain one valid load segment in " +
                                 displayPath(path));
    }

    bool hasIopMod = false;
    for(uint16_t index = 0; index < sectionCount; ++index)
    {
        const uint8_t* header = irx.data() + sectionOffset + index * SECTION_HEADER_SIZE;
        const uint32_t type = readU32(header + 4);
        const uint32_t dataOffset = readU32(header + 16);
        const uint32_t dataSize = readU32(header + 20);
        if(type != SHT_NOBITS && !checkedRange(dataOffset, dataSize, irx.size()))
        {
            throw std::runtime_error("psf2.irx section exceeds its file in " + displayPath(path));
        }
        if(type == IOPMOD_SECTION)
        {
            if(dataSize < IOPMOD_SIZE ||
               std::memchr(irx.data() + dataOffset + 26, 0, 256) == nullptr)
            {
                throw std::runtime_error("psf2.irx has an invalid IOP module section in " +
                                         displayPath(path));
            }
            const uint64_t text = readU32(irx.data() + dataOffset + 12);
            const uint64_t data = readU32(irx.data() + dataOffset + 16);
            const uint64_t bss = readU32(irx.data() + dataOffset + 20);
            if(text + data > loadMemorySize || (bss != 0 && text + data + bss != loadMemorySize))
            {
                throw std::runtime_error("psf2.irx module sizes are inconsistent in " +
                                         displayPath(path));
            }
            hasIopMod = true;
        }
    }
    if(!hasIopMod)
    {
        throw std::runtime_error("psf2.irx has no IOP module section in " + displayPath(path));
    }
}

void validateDirectory(const Bytes& reserved, uint32_t offset, unsigned int depth,
                       std::set<uint32_t>& activeDirectories, ValidationContext& context,
                       const fs::path& path, const std::string& prefix, bool& hasRootIrx)
{
    if(depth > MAX_FS_DEPTH || !activeDirectories.insert(offset).second)
    {
        throw std::runtime_error("PSF2 filesystem recursion is invalid in " + displayPath(path));
    }
    if(!checkedRange(offset, 4, reserved.size()))
    {
        throw std::runtime_error("PSF2 directory header exceeds reserved data in " +
                                 displayPath(path));
    }
    const uint32_t count = readU32(reserved.data() + offset);
    if(count > MAX_FS_ENTRIES - context.totalFsEntries ||
       !checkedRange(static_cast<uint64_t>(offset) + 4,
                     static_cast<uint64_t>(count) * 48U, reserved.size()))
    {
        throw std::runtime_error("PSF2 directory table exceeds Kog's bounds in " +
                                 displayPath(path));
    }
    context.totalFsEntries += count;

    for(uint32_t index = 0; index < count; ++index)
    {
        const uint8_t* entry = reserved.data() + offset + 4U + index * 48U;
        size_t nameLength = 0;
        while(nameLength < 36 && entry[nameLength] != 0) ++nameLength;
        std::string name(reinterpret_cast<const char*>(entry), nameLength);
        if(name.empty() || name == "." || name == ".." || name.find('/') != std::string::npos ||
           name.find('\\') != std::string::npos)
        {
            throw std::runtime_error("PSF2 filesystem contains an invalid name in " +
                                     displayPath(path));
        }
        const uint32_t nodeOffset = readU32(entry + 36);
        const uint32_t size = readU32(entry + 40);
        const uint32_t blockSize = readU32(entry + 44);
        const std::string nodePath = prefix.empty() ? name : prefix + "/" + name;
        if(blockSize == 0)
        {
            validateDirectory(reserved, nodeOffset, depth + 1, activeDirectories, context, path,
                              nodePath, hasRootIrx);
            continue;
        }
        if(size > MAX_FS_FILE_BYTES || blockSize > MAX_FS_FILE_BYTES ||
           context.totalFsBytes > MAX_FS_TOTAL_BYTES - size)
        {
            throw std::runtime_error("PSF2 filesystem file exceeds Kog's bounds in " +
                                     displayPath(path));
        }
        context.totalFsBytes += size;
        const uint64_t blockCount =
            (static_cast<uint64_t>(size) + blockSize - 1U) / blockSize;
        if(blockCount > MAX_FS_ENTRIES ||
           !checkedRange(nodeOffset, blockCount * sizeof(uint32_t), reserved.size()))
        {
            throw std::runtime_error("PSF2 compressed block table is invalid in " +
                                     displayPath(path));
        }
        uint64_t cursor = static_cast<uint64_t>(nodeOffset) + blockCount * sizeof(uint32_t);
        uint64_t produced = 0;
        Bytes fileData;
        if(nodePath == "psf2.irx") fileData.reserve(size);
        for(uint64_t block = 0; block < blockCount; ++block)
        {
            const uint32_t compressedSize =
                readU32(reserved.data() + nodeOffset + block * sizeof(uint32_t));
            if(compressedSize == 0 || compressedSize > MAX_COMPRESSED_BLOCK_BYTES ||
               !checkedRange(cursor, compressedSize, reserved.size()))
            {
                throw std::runtime_error("PSF2 compressed block exceeds Kog's bounds in " +
                                         displayPath(path));
            }
            const uint64_t expected = std::min<uint64_t>(blockSize, size - produced);
            Bytes output(static_cast<size_t>(expected));
            uLongf outputSize = static_cast<uLongf>(expected);
            const int result = uncompress(output.data(), &outputSize, reserved.data() + cursor,
                                          compressedSize);
            if(result != Z_OK || outputSize != expected)
            {
                throw std::runtime_error("invalid PSF2 compressed filesystem block in " +
                                         displayPath(path));
            }
            if(nodePath == "psf2.irx")
            {
                fileData.insert(fileData.end(), output.begin(), output.end());
            }
            cursor += compressedSize;
            produced += expected;
        }
        if(produced != size)
        {
            throw std::runtime_error("PSF2 filesystem file is truncated in " + displayPath(path));
        }
        if(nodePath == "psf2.irx")
        {
            validateIrx(fileData, path);
            hasRootIrx = true;
        }
    }
    activeDirectories.erase(offset);
}

ParsedPsf validatePsf(const fs::path& path, ValidationContext& context)
{
    const Bytes bytes = readFile(path);
    if(bytes.size() < 16 || std::memcmp(bytes.data(), "PSF\x02", 4) != 0)
    {
        throw std::runtime_error("unsupported or truncated PSF2 file: " + displayPath(path));
    }
    const uint32_t reservedLength = readU32(bytes.data() + 4);
    const uint32_t compressedLength = readU32(bytes.data() + 8);
    const uint32_t expectedCrc = readU32(bytes.data() + 12);
    const uint64_t dataEnd = 16ULL + reservedLength + compressedLength;
    if(reservedLength == 0 || reservedLength > MAX_RESERVED_BYTES || dataEnd > bytes.size())
    {
        throw std::runtime_error("PSF2 section lengths are invalid in " + displayPath(path));
    }
    validateCompressedProgram(bytes.data() + 16U + reservedLength, compressedLength, expectedCrc,
                              path);

    Bytes reserved(bytes.begin() + 16, bytes.begin() + 16 + reservedLength);
    std::set<uint32_t> activeDirectories;
    ParsedPsf parsed;
    validateDirectory(reserved, 0, 0, activeDirectories, context, path, "", parsed.hasRootIrx);
    parsed.tags = parseTags(bytes, static_cast<size_t>(dataEnd), path);
    return parsed;
}

fs::path dependencyPath(const fs::path& parent, const std::string& name)
{
    if(name.empty() || name.find('\0') != std::string::npos)
    {
        throw std::runtime_error("empty PSF2 library tag");
    }
#ifdef _WIN32
    const fs::path relative = fs::u8path(name);
#else
    const fs::path relative(name);
#endif
    if(relative.is_absolute())
    {
        throw std::runtime_error("absolute PSF2 library paths are not accepted");
    }
    return parent.parent_path() / relative;
}

std::string canonicalKey(const fs::path& path)
{
    std::error_code error;
    const fs::path canonical = fs::weakly_canonical(path, error);
    return displayPath(error ? path.lexically_normal() : canonical);
}

ParsedPsf validateLibraries(const fs::path& path, unsigned int depth, ValidationContext& context)
{
    if(depth > MAX_LIBRARY_DEPTH)
    {
        throw std::runtime_error("PSF2 library nesting exceeds sixteen levels");
    }
    const std::string key = canonicalKey(path);
    if(context.activeLibraries.count(key) != 0)
    {
        throw std::runtime_error("PSF2 library dependency cycle includes " + displayPath(path));
    }
    if(context.loadedLibraries.count(key) != 0) return {};
    context.activeLibraries.insert(key);
    ParsedPsf parsed = validatePsf(path, context);

    const auto primary = parsed.tags.find("_lib");
    if(primary != parsed.tags.end())
    {
        validateLibraries(dependencyPath(path, primary->second), depth + 1, context);
    }
    context.loadedLibraries.insert(key);
    context.loadOrder.push_back(path);
    context.hasRootIrx = context.hasRootIrx || parsed.hasRootIrx;
    for(const auto& tag : parsed.tags)
    {
        context.fallbackTags.emplace(tag.first, tag.second);
    }
    for(unsigned int index = 2; index <= 9; ++index)
    {
        const auto auxiliary = parsed.tags.find("_lib" + std::to_string(index));
        if(auxiliary != parsed.tags.end())
        {
            validateLibraries(dependencyPath(path, auxiliary->second), depth + 1, context);
        }
    }
    context.activeLibraries.erase(key);
    return parsed;
}

uint32_t parseMilliseconds(const std::string& text)
{
    if(text.empty()) return 0;
    const char* component = text.c_str();
    long double totalSeconds = 0;
    while(true)
    {
        char* end = nullptr;
        errno = 0;
        const long double value = std::strtold(component, &end);
        if(errno != 0 || end == component || value < 0 || !std::isfinite(value) ||
           (*end != ':' && *end != '\0'))
        {
            return 0;
        }
        totalSeconds = totalSeconds * 60 + value;
        if(*end == '\0') break;
        component = end + 1;
    }
    if(totalSeconds <= 0 || totalSeconds * 1000 > MAX_DURATION_MILLISECONDS) return 0;
    return static_cast<uint32_t>(totalSeconds * 1000 + 0.5L);
}

uint64_t parseU64(const char* text, const char* label)
{
    if(text == nullptr || *text == '\0' || *text == '-')
    {
        throw std::runtime_error(std::string("invalid ") + label);
    }
    char* end = nullptr;
    errno = 0;
    const unsigned long long value = std::strtoull(text, &end, 10);
    if(errno != 0 || end == text || *end != '\0')
    {
        throw std::runtime_error(std::string("invalid ") + label);
    }
    return static_cast<uint64_t>(value);
}

bool writeU32(uint32_t value)
{
    const uint8_t bytes[4] = {static_cast<uint8_t>(value), static_cast<uint8_t>(value >> 8U),
                              static_cast<uint8_t>(value >> 16U),
                              static_cast<uint8_t>(value >> 24U)};
    return std::fwrite(bytes, 1, sizeof(bytes), stdout) == sizeof(bytes);
}

bool writeU64(uint64_t value)
{
    uint8_t bytes[8] = {};
    for(unsigned int index = 0; index < 8; ++index)
    {
        bytes[index] = static_cast<uint8_t>(value >> (index * 8U));
    }
    return std::fwrite(bytes, 1, sizeof(bytes), stdout) == sizeof(bytes);
}

std::string metadata(const Tags& tags, const char* name)
{
    const auto value = tags.find(name);
    if(value == tags.end() || lowerAscii(value->second) == "n/a") return {};
    return value->second;
}

bool writeHeader(const Tags& tags, uint64_t mainFrames, uint64_t totalFrames)
{
    std::array<std::string, 5> fields = {
        metadata(tags, "title"), metadata(tags, "artist"), metadata(tags, "game"),
        metadata(tags, "genre"), metadata(tags, "date")};
    if(fields[4].empty()) fields[4] = metadata(tags, "year");
    uint64_t metadataBytes = 0;
    for(const auto& field : fields) metadataBytes += field.size();
    if(metadataBytes > 64U * 1024U) return false;
    if(std::fwrite(HELPER_MAGIC.data(), 1, HELPER_MAGIC.size(), stdout) != HELPER_MAGIC.size() ||
       !writeU32(1) || !writeU32(2) || !writeU32(SAMPLE_RATE) || !writeU32(CHANNELS) ||
       !writeU64(totalFrames) || !writeU64(mainFrames))
    {
        return false;
    }
    for(const auto& field : fields)
    {
        if(field.size() > std::numeric_limits<uint32_t>::max() ||
           !writeU32(static_cast<uint32_t>(field.size())))
        {
            return false;
        }
    }
    for(const auto& field : fields)
    {
        if(!field.empty() && std::fwrite(field.data(), 1, field.size(), stdout) != field.size())
        {
            return false;
        }
    }
    return std::fflush(stdout) == 0;
}

class StreamSoundHandler final : public CSoundHandler
{
public:
    StreamSoundHandler(uint64_t startFrame, uint64_t totalFrames)
        : m_startFrame(startFrame)
        , m_totalFrames(totalFrames)
    {
    }

    void Reset() override
    {
        m_sourceFrame = 0;
        m_failed = false;
    }

    void Write(int16* samples, unsigned int sampleCount, unsigned int sampleRate) override
    {
        if(m_failed || sampleRate != SAMPLE_RATE || (sampleCount % CHANNELS) != 0)
        {
            m_failed = true;
            return;
        }
        const uint64_t frames = sampleCount / CHANNELS;
        const uint64_t begin = std::max(m_sourceFrame, m_startFrame);
        const uint64_t end = std::min(m_sourceFrame + frames, m_totalFrames);
        if(begin < end)
        {
            const size_t firstSample = static_cast<size_t>(begin - m_sourceFrame) * CHANNELS;
            const size_t outputSamples = static_cast<size_t>(end - begin) * CHANNELS;
            std::array<uint8_t, 4096> bytes = {};
            size_t written = 0;
            while(written < outputSamples)
            {
                const size_t chunk = std::min(outputSamples - written, bytes.size() / 2);
                for(size_t index = 0; index < chunk; ++index)
                {
                    const uint16_t sample = static_cast<uint16_t>(samples[firstSample + written + index]);
                    bytes[index * 2] = static_cast<uint8_t>(sample);
                    bytes[index * 2 + 1] = static_cast<uint8_t>(sample >> 8U);
                }
                if(std::fwrite(bytes.data(), 2, chunk, stdout) != chunk)
                {
                    m_failed = true;
                    return;
                }
                written += chunk;
            }
        }
        m_sourceFrame += frames;
    }

    bool HasFreeBuffers() override
    {
        return true;
    }

    void RecycleBuffers() override
    {
    }

    bool done() const
    {
        return m_sourceFrame >= m_totalFrames || m_failed;
    }

    bool failed() const
    {
        return m_failed;
    }

private:
    uint64_t m_startFrame = 0;
    uint64_t m_totalFrames = 0;
    uint64_t m_sourceFrame = 0;
    bool m_failed = false;
};

void loadArchives(PS2::CPsfDevice& device, const std::vector<fs::path>& paths)
{
    for(const auto& path : paths)
    {
        auto stream = Framework::CreateInputStdStream(path.native());
        CPsfBase psf(stream);
        if(psf.GetVersion() != CPsfBase::VERSION_PLAYSTATION2)
        {
            throw std::runtime_error("PSF2 library version changed after validation");
        }
        device.AppendArchive(psf);
    }
}

int runHelper(const fs::path& path, const char* startText, const char* defaultLengthText,
              const char* defaultFadeText)
{
    uint64_t startFrame = parseU64(startText, "PSF2 start frame");
    const uint64_t defaultLength = parseU64(defaultLengthText, "PSF2 default length");
    const uint64_t defaultFade = parseU64(defaultFadeText, "PSF2 default fade");
    if(defaultLength == 0 || defaultLength > MAX_DURATION_MILLISECONDS ||
       defaultFade > MAX_DURATION_MILLISECONDS)
    {
        throw std::runtime_error("PSF2 default duration exceeds Kog's limit");
    }

    ValidationContext context;
    ParsedPsf root = validateLibraries(path, 0, context);
    if(!context.hasRootIrx)
    {
        throw std::runtime_error("PSF2 library chain contains no root psf2.irx");
    }
    Tags tags = context.fallbackTags;
    for(const auto& tag : root.tags) tags[tag.first] = tag.second;

    uint32_t lengthMilliseconds = 0;
    uint32_t fadeMilliseconds = 0;
    const auto length = tags.find("length");
    if(length != tags.end()) lengthMilliseconds = parseMilliseconds(length->second);
    if(lengthMilliseconds == 0)
    {
        lengthMilliseconds = static_cast<uint32_t>(defaultLength);
        fadeMilliseconds = static_cast<uint32_t>(defaultFade);
    }
    else
    {
        const auto fade = tags.find("fade");
        if(fade != tags.end()) fadeMilliseconds = parseMilliseconds(fade->second);
    }
    const uint64_t mainFrames =
        static_cast<uint64_t>(lengthMilliseconds) * SAMPLE_RATE / 1000U;
    const uint64_t fadeFrames =
        static_cast<uint64_t>(fadeMilliseconds) * SAMPLE_RATE / 1000U;
    if(mainFrames == 0 || mainFrames > std::numeric_limits<uint64_t>::max() - fadeFrames)
    {
        throw std::runtime_error("invalid PSF2 duration metadata");
    }
    const uint64_t totalFrames = mainFrames + fadeFrames;
    startFrame = std::min(startFrame, totalFrames);

    Iop::CPsfSubSystem subsystem(true);
    auto* bios = dynamic_cast<CIopBios*>(subsystem.GetBios());
    if(bios == nullptr) throw std::runtime_error("Play! did not create its PS2 IOP HLE BIOS");
    bios->Reset(PS2::IOP_BASE_RAM_SIZE, std::shared_ptr<Iop::CSifMan>());
    auto device = std::make_shared<PS2::CPsfDevice>();
    auto ioman = bios->GetIoman();
    ioman->RegisterDevice("psf", device);
    ioman->RegisterDevice("host0", device);
    ioman->RegisterDevice("hefile", device);
    loadArchives(*device, context.loadOrder);

    constexpr const char* EXECUTABLE_PATH = "psf:/psf2.irx";
    const int32_t module = bios->LoadModuleFromPath(EXECUTABLE_PATH);
    if(module < 0 ||
       bios->StartModule(CIopBios::MODULESTARTREQUEST_SOURCE::REMOTE, module, EXECUTABLE_PATH,
                         nullptr, 0) < 0)
    {
        throw std::runtime_error("Play! could not load psf2.irx");
    }
    if(!writeHeader(tags, mainFrames, totalFrames))
    {
        throw std::runtime_error("writing the PSF2 stream header failed");
    }

    StreamSoundHandler sound(startFrame, totalFrames);
    while(!sound.done()) subsystem.Update(false, &sound);
    if(sound.failed()) throw std::runtime_error("writing PSF2 PCM failed");
    return 0;
}
} // namespace

#ifdef _WIN32
int wmain(int argc, wchar_t** argv)
{
    _setmode(_fileno(stdout), _O_BINARY);
    if(argc != 5)
    {
        std::fprintf(stderr,
                     "usage: kog-psf2-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
        return 2;
    }
    try
    {
        const auto narrowArgument = [](const wchar_t* value) {
            const int length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, -1,
                                                    nullptr, 0, nullptr, nullptr);
            if(length <= 0) throw std::runtime_error("invalid UTF-16 helper argument");
            std::string result(static_cast<size_t>(length), '\0');
            WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, -1, result.data(), length,
                                nullptr, nullptr);
            result.pop_back();
            return result;
        };
        const std::string start = narrowArgument(argv[2]);
        const std::string length = narrowArgument(argv[3]);
        const std::string fade = narrowArgument(argv[4]);
        return runHelper(fs::path(argv[1]), start.c_str(), length.c_str(), fade.c_str());
    }
    catch(const std::exception& error)
    {
        std::fprintf(stderr, "%s\n", error.what());
        return 3;
    }
}
#else
int main(int argc, char** argv)
{
    if(argc != 5)
    {
        std::fprintf(stderr,
                     "usage: kog-psf2-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
        return 2;
    }
    try
    {
        return runHelper(fs::path(argv[1]), argv[2], argv[3], argv[4]);
    }
    catch(const std::exception& error)
    {
        std::fprintf(stderr, "%s\n", error.what());
        return 3;
    }
}
#endif
