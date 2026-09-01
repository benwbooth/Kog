/*
 * Kog 2SF helper process.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * psflib supplies xSF dependency traversal and decompression. The maintained
 * melonDS core supplies the Nintendo DS implementation. This file is only the
 * bounded 2SF map adapter and Kog's process protocol.
 */

#include <algorithm>
#include <array>
#include <cerrno>
#include <climits>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <limits>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#endif

#include <zlib.h>

#include "Args.h"
#include "NDS.h"
#include "NDSCart.h"
#include "SPI.h"
#include "SPU.h"
#include "psflib.h"

namespace fs = std::filesystem;

namespace
{
constexpr uint64_t MAX_FILE_BYTES = 512ULL * 1024ULL * 1024ULL;
constexpr uint64_t MAX_ROM_BYTES = 512ULL * 1024ULL * 1024ULL;
constexpr uint64_t MAX_SAVE_BYTES = 128ULL * 1024ULL * 1024ULL;
constexpr uint32_t MAX_DURATION_MILLISECONDS = 24U * 60U * 60U * 1000U;
// Cog derives this from the 33,513,982 Hz ARM7 clock divided by 1024.
constexpr uint32_t SAMPLE_RATE = 32728U;
constexpr uint32_t CHANNELS = 2U;
constexpr uint32_t FORMAT_VERSION = 0x24U;
constexpr std::array<uint8_t, 8> HELPER_MAGIC = {'K', 'O', 'G', 'P', 'S', 'F', '1', 0};

using Bytes = std::vector<uint8_t>;
using Tags = std::map<std::string, std::string>;

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
            character = static_cast<char>(character - 'A' + 'a');
    }
    return text;
}

struct InputFile
{
    std::FILE* stream = nullptr;
};

void* sourceOpen(void*, const char* path)
{
    if(path == nullptr) return nullptr;
#ifdef _WIN32
    std::FILE* stream = _wfopen(fs::u8path(path).c_str(), L"rb");
#else
    std::FILE* stream = std::fopen(path, "rb");
#endif
    if(stream == nullptr) return nullptr;
#ifdef _WIN32
    if(_fseeki64(stream, 0, SEEK_END) != 0)
    {
        std::fclose(stream);
        return nullptr;
    }
    const auto length = _ftelli64(stream);
    _fseeki64(stream, 0, SEEK_SET);
#else
    if(fseeko(stream, 0, SEEK_END) != 0)
    {
        std::fclose(stream);
        return nullptr;
    }
    const auto length = ftello(stream);
    fseeko(stream, 0, SEEK_SET);
#endif
    if(length < 0 || static_cast<uint64_t>(length) > MAX_FILE_BYTES)
    {
        std::fclose(stream);
        return nullptr;
    }
    return new InputFile {stream};
}

size_t sourceRead(void* buffer, size_t size, size_t count, void* handle)
{
    const auto* input = static_cast<InputFile*>(handle);
    if(input == nullptr || input->stream == nullptr) return 0;
    return std::fread(buffer, size, count, input->stream);
}

int sourceSeek(void* handle, int64_t offset, int origin)
{
    const auto* input = static_cast<InputFile*>(handle);
    if(input == nullptr || input->stream == nullptr) return -1;
#ifdef _WIN32
    return _fseeki64(input->stream, offset, origin);
#else
    return fseeko(input->stream, static_cast<off_t>(offset), origin);
#endif
}

int sourceClose(void* handle)
{
    auto* input = static_cast<InputFile*>(handle);
    if(input == nullptr) return -1;
    const int result = input->stream == nullptr ? -1 : std::fclose(input->stream);
    delete input;
    return result;
}

long sourceTell(void* handle)
{
    const auto* input = static_cast<InputFile*>(handle);
    if(input == nullptr || input->stream == nullptr) return -1;
#ifdef _WIN32
    const auto position = _ftelli64(input->stream);
#else
    const auto position = ftello(input->stream);
#endif
    if(position < 0 || static_cast<uint64_t>(position) > static_cast<uint64_t>(LONG_MAX))
        return -1;
    return static_cast<long>(position);
}

const psf_file_callbacks FILE_CALLBACKS = {
    "/|\\", nullptr, sourceOpen, sourceRead, sourceSeek, sourceClose, sourceTell};

struct LoadState
{
    Bytes rom;
    Bytes save;
    Tags tags;
    int initialFrames = 0;
    std::string error;
};

size_t roundedRomSize(uint64_t minimum)
{
    if(minimum == 0 || minimum > MAX_ROM_BYTES)
        throw std::runtime_error("2SF ROM mapping exceeds Kog's limit");
    uint64_t result = 1;
    while(result < minimum) result <<= 1U;
    if(result > MAX_ROM_BYTES || result > std::numeric_limits<size_t>::max())
        throw std::runtime_error("2SF ROM mapping exceeds this platform's limit");
    return static_cast<size_t>(result);
}

void mapSection(LoadState& state, bool save, const uint8_t* data, size_t dataSize)
{
    if(data == nullptr || dataSize < 8)
        throw std::runtime_error("2SF mapping is shorter than its header");
    const uint64_t offset = readU32(data);
    const uint64_t size = readU32(data + 4);
    if(size > dataSize - 8)
        throw std::runtime_error("2SF mapping payload exceeds its decompressed section");
    const uint64_t limit = save ? MAX_SAVE_BYTES : MAX_ROM_BYTES;
    if(offset > limit || size > limit - offset)
        throw std::runtime_error(save ? "2SF save mapping exceeds Kog's limit"
                                      : "2SF ROM mapping exceeds Kog's limit");
    if(size == 0) return;

    Bytes& destination = save ? state.save : state.rom;
    const uint64_t required = offset + size;
    const size_t allocation = save ? static_cast<size_t>(required) : roundedRomSize(required);
    if(destination.size() < allocation) destination.resize(allocation, 0);
    std::copy_n(data + 8, static_cast<size_t>(size),
                destination.begin() + static_cast<ptrdiff_t>(offset));
}

Bytes inflateSave(const uint8_t* compressed, size_t compressedSize, uint32_t expectedCrc)
{
    if(compressed == nullptr || compressedSize == 0 || compressedSize > MAX_FILE_BYTES)
        throw std::runtime_error("2SF SAVE block has an invalid compressed size");
    z_stream inflater = {};
    inflater.next_in = const_cast<Bytef*>(compressed);
    inflater.avail_in = static_cast<uInt>(compressedSize);
    if(inflateInit(&inflater) != Z_OK)
        throw std::runtime_error("cannot initialize 2SF SAVE decompression");

    Bytes output;
    std::array<uint8_t, 16384> buffer = {};
    int result = Z_OK;
    while(result == Z_OK)
    {
        inflater.next_out = buffer.data();
        inflater.avail_out = static_cast<uInt>(buffer.size());
        result = inflate(&inflater, Z_NO_FLUSH);
        const size_t produced = buffer.size() - inflater.avail_out;
        if(output.size() > MAX_SAVE_BYTES - produced)
        {
            inflateEnd(&inflater);
            throw std::runtime_error("2SF SAVE data exceeds Kog's limit");
        }
        output.insert(output.end(), buffer.begin(), buffer.begin() + static_cast<ptrdiff_t>(produced));
    }
    const bool valid = result == Z_STREAM_END && inflater.avail_in == 0;
    inflateEnd(&inflater);
    if(!valid) throw std::runtime_error("2SF SAVE data is not a complete zlib stream");
    if(static_cast<uint32_t>(crc32(0, output.data(), static_cast<uInt>(output.size()))) !=
       expectedCrc)
        throw std::runtime_error("2SF SAVE data has a CRC mismatch");
    return output;
}

int loadSections(void* context, const uint8_t* executable, size_t executableSize,
                 const uint8_t* reserved, size_t reservedSize)
{
    auto& state = *static_cast<LoadState*>(context);
    try
    {
        if(executableSize != 0) mapSection(state, false, executable, executableSize);
        size_t position = 0;
        while(position < reservedSize)
        {
            if(!checkedRange(position, 12, reservedSize))
                throw std::runtime_error("2SF reserved block has a truncated header");
            const uint32_t type = readU32(reserved + position);
            const uint32_t compressedSize = readU32(reserved + position + 4);
            const uint32_t crc = readU32(reserved + position + 8);
            position += 12;
            if(!checkedRange(position, compressedSize, reservedSize))
                throw std::runtime_error("2SF reserved block exceeds its section");
            if(type == 0x45564153U)
            {
                const Bytes save = inflateSave(reserved + position, compressedSize, crc);
                mapSection(state, true, save.data(), save.size());
            }
            position += compressedSize;
        }
        return 0;
    }
    catch(const std::exception& error)
    {
        state.error = error.what();
        return -1;
    }
}

int loadTag(void* context, const char* name, const char* value)
{
    auto& state = *static_cast<LoadState*>(context);
    if(name == nullptr || value == nullptr) return 0;
    std::string key = lowerAscii(name);
    if(key.size() > 128 || std::strlen(value) > 16U * 1024U)
    {
        state.error = "2SF metadata exceeds Kog's limit";
        return -1;
    }
    state.tags[key] = value;
    if(key == "_frames")
    {
        char* end = nullptr;
        const long frames = std::strtol(value, &end, 10);
        if(end != value && *end == '\0' && frames >= 0 && frames <= 60L * 60L * 24L)
            state.initialFrames = static_cast<int>(frames);
    }
    return 0;
}

void loadStatus(void* context, const char* message)
{
    auto& state = *static_cast<LoadState*>(context);
    if(state.error.empty() && message != nullptr) state.error = message;
}

void validateRom(const Bytes& rom)
{
    // NDSCart::ParseROM reads the complete 4 KiB Nintendo DS header.
    if(rom.size() < 0x1000 || rom.size() > MAX_ROM_BYTES)
        throw std::runtime_error("2SF library chain did not produce a bounded Nintendo DS ROM");
    struct Program
    {
        uint32_t romOffset;
        uint32_t entry;
        uint32_t ramAddress;
        uint32_t size;
        const char* name;
    };
    const std::array<Program, 2> programs = {{
        {readU32(rom.data() + 0x20), readU32(rom.data() + 0x24),
         readU32(rom.data() + 0x28), readU32(rom.data() + 0x2C), "ARM9"},
        {readU32(rom.data() + 0x30), readU32(rom.data() + 0x34),
         readU32(rom.data() + 0x38), readU32(rom.data() + 0x3C), "ARM7"},
    }};
    for(const auto& program : programs)
    {
        const uint64_t ramEnd = static_cast<uint64_t>(program.ramAddress) + program.size;
        if(program.size == 0 || (program.size & 3U) != 0 ||
           !checkedRange(program.romOffset, program.size, rom.size()) ||
           ramEnd > std::numeric_limits<uint32_t>::max() + 1ULL ||
           program.entry < program.ramAddress || program.entry >= ramEnd)
            throw std::runtime_error(std::string("2SF has invalid ") + program.name +
                                     " executable ranges");
    }
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
            return 0;
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
        throw std::runtime_error(std::string("invalid ") + label);
    char* end = nullptr;
    errno = 0;
    const unsigned long long value = std::strtoull(text, &end, 10);
    if(errno != 0 || end == text || *end != '\0')
        throw std::runtime_error(std::string("invalid ") + label);
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
        bytes[index] = static_cast<uint8_t>(value >> (index * 8U));
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
    if(fields[2].empty()) fields[2] = metadata(tags, "album");
    if(fields[4].empty()) fields[4] = metadata(tags, "year");
    uint64_t metadataBytes = 0;
    for(const auto& field : fields) metadataBytes += field.size();
    if(metadataBytes > 64U * 1024U) return false;
    if(std::fwrite(HELPER_MAGIC.data(), 1, HELPER_MAGIC.size(), stdout) != HELPER_MAGIC.size() ||
       !writeU32(1) || !writeU32(FORMAT_VERSION) || !writeU32(SAMPLE_RATE) ||
       !writeU32(CHANNELS) || !writeU64(totalFrames) || !writeU64(mainFrames))
        return false;
    for(const auto& field : fields)
    {
        if(field.size() > std::numeric_limits<uint32_t>::max() ||
           !writeU32(static_cast<uint32_t>(field.size()))) return false;
    }
    for(const auto& field : fields)
    {
        if(!field.empty() && std::fwrite(field.data(), 1, field.size(), stdout) != field.size())
            return false;
    }
    return std::fflush(stdout) == 0;
}

void writeSamples(const int16_t* samples, size_t frameCount)
{
    std::array<uint8_t, 8192> bytes = {};
    size_t frame = 0;
    while(frame < frameCount)
    {
        const size_t chunkFrames = std::min(frameCount - frame, bytes.size() / (CHANNELS * 2U));
        for(size_t sample = 0; sample < chunkFrames * CHANNELS; ++sample)
        {
            const uint16_t value = static_cast<uint16_t>(samples[frame * CHANNELS + sample]);
            bytes[sample * 2] = static_cast<uint8_t>(value);
            bytes[sample * 2 + 1] = static_cast<uint8_t>(value >> 8U);
        }
        if(std::fwrite(bytes.data(), CHANNELS * 2U, chunkFrames, stdout) != chunkFrames)
            throw std::runtime_error("writing 2SF PCM failed");
        frame += chunkFrames;
    }
}

int runHelper(const std::string& path, const char* startText, const char* defaultLengthText,
              const char* defaultFadeText)
{
    uint64_t startFrame = parseU64(startText, "2SF start frame");
    const uint64_t defaultLength = parseU64(defaultLengthText, "2SF default length");
    const uint64_t defaultFade = parseU64(defaultFadeText, "2SF default fade");
    if(defaultLength == 0 || defaultLength > MAX_DURATION_MILLISECONDS ||
       defaultFade > MAX_DURATION_MILLISECONDS)
        throw std::runtime_error("2SF default duration exceeds Kog's limit");

    LoadState state;
    if(psf_load(path.c_str(), &FILE_CALLBACKS, 0x24, loadSections, &state, loadTag, &state, 1,
                loadStatus, &state) != 0x24)
    {
        throw std::runtime_error(state.error.empty() ? "psflib rejected the 2SF dependency chain"
                                                      : state.error);
    }
    // psflib reports nested tags as well when requested for compatibility
    // controls such as _frames. Re-read only the outer file's tags so a
    // mini2SF title and timing always override its shared library metadata.
    if(psf_load(path.c_str(), &FILE_CALLBACKS, 0x24, nullptr, nullptr, loadTag, &state, 0,
                nullptr, nullptr) != 0x24)
        throw std::runtime_error("psflib could not read the outer 2SF metadata");
    validateRom(state.rom);

    uint32_t lengthMilliseconds = 0;
    uint32_t fadeMilliseconds = 0;
    const auto length = state.tags.find("length");
    if(length != state.tags.end()) lengthMilliseconds = parseMilliseconds(length->second);
    if(lengthMilliseconds == 0)
    {
        lengthMilliseconds = static_cast<uint32_t>(defaultLength);
        fadeMilliseconds = static_cast<uint32_t>(defaultFade);
    }
    else
    {
        const auto fade = state.tags.find("fade");
        if(fade != state.tags.end()) fadeMilliseconds = parseMilliseconds(fade->second);
    }
    const uint64_t mainFrames = static_cast<uint64_t>(lengthMilliseconds) * SAMPLE_RATE / 1000U;
    const uint64_t fadeFrames = static_cast<uint64_t>(fadeMilliseconds) * SAMPLE_RATE / 1000U;
    if(mainFrames == 0 || mainFrames > std::numeric_limits<uint64_t>::max() - fadeFrames)
        throw std::runtime_error("invalid 2SF duration metadata");
    const uint64_t totalFrames = mainFrames + fadeFrames;
    startFrame = std::min(startFrame, totalFrames);

    auto rom = std::make_unique<melonDS::u8[]>(state.rom.size());
    std::copy(state.rom.begin(), state.rom.end(), rom.get());
    auto cart = melonDS::NDSCart::ParseROM(std::move(rom),
        static_cast<melonDS::u32>(state.rom.size()));
    if(cart == nullptr) throw std::runtime_error("melonDS rejected the 2SF Nintendo DS ROM");

    melonDS::NDSArgs arguments;
    arguments.JIT = std::nullopt;
    arguments.BitDepth = melonDS::AudioBitDepth::Auto;
    arguments.Interpolation = melonDS::AudioInterpolation::None;
    arguments.OutputSampleRate = static_cast<double>(SAMPLE_RATE);
    auto nds = std::make_unique<melonDS::NDS>(std::move(arguments));
    nds->SetNDSCart(std::move(cart));
    nds->SetGBACart(nullptr);
    nds->Reset();
    nds->SPI.GetPowerMan()->SetBatteryLevelOkay(true);
    if(!state.save.empty())
        nds->SetNDSSave(state.save.data(), static_cast<melonDS::u32>(state.save.size()));
    if(nds->NeedsDirectBoot()) nds->SetupDirectBoot("kog-2sf.nds");
    nds->Start();

    for(int frame = 0; frame < state.initialFrames; ++frame)
    {
        nds->RunFrame();
        nds->SPU.DrainOutput();
    }
    if(!writeHeader(state.tags, mainFrames, totalFrames))
        throw std::runtime_error("writing the 2SF stream header failed");

    uint64_t sourceFrame = 0;
    unsigned int emptyFrames = 0;
    std::vector<int16_t> samples(2048U * CHANNELS);
    while(sourceFrame < totalFrames)
    {
        int available = nds->SPU.GetOutputSize();
        if(available == 0)
        {
            if(!nds->IsRunning()) throw std::runtime_error("melonDS stopped before 2SF playback ended");
            nds->RunFrame();
            available = nds->SPU.GetOutputSize();
            if(available == 0)
            {
                if(++emptyFrames > 600U)
                    throw std::runtime_error("2SF produced no audio for 600 Nintendo DS frames");
                continue;
            }
            emptyFrames = 0;
        }
        const uint64_t wanted = std::min<uint64_t>(
            static_cast<uint64_t>(available), totalFrames - sourceFrame);
        const int chunk = static_cast<int>(std::min<uint64_t>(wanted, samples.size() / CHANNELS));
        const int received = nds->SPU.ReadOutput(samples.data(), chunk);
        if(received <= 0) continue;
        const uint64_t begin = std::max(sourceFrame, startFrame);
        const uint64_t end = sourceFrame + static_cast<uint64_t>(received);
        if(begin < end)
        {
            const size_t skip = static_cast<size_t>(begin - sourceFrame);
            writeSamples(samples.data() + skip * CHANNELS,
                         static_cast<size_t>(end - begin));
        }
        sourceFrame = end;
    }
    nds->Stop();
    return 0;
}
}

#ifdef _WIN32
int wmain(int argc, wchar_t** argv)
{
    _setmode(_fileno(stdout), _O_BINARY);
    if(argc != 5)
    {
        std::fprintf(stderr,
                     "usage: kog-2sf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
        return 2;
    }
    try
    {
        const auto narrow = [](const wchar_t* value) {
            const int length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, -1,
                                                    nullptr, 0, nullptr, nullptr);
            if(length <= 0) throw std::runtime_error("invalid UTF-16 helper argument");
            std::string result(static_cast<size_t>(length), '\0');
            WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, value, -1, result.data(), length,
                                nullptr, nullptr);
            result.pop_back();
            return result;
        };
        const std::string path = narrow(argv[1]);
        const std::string start = narrow(argv[2]);
        const std::string length = narrow(argv[3]);
        const std::string fade = narrow(argv[4]);
        return runHelper(path, start.c_str(), length.c_str(), fade.c_str());
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
                     "usage: kog-2sf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
        return 2;
    }
    try
    {
        return runHelper(argv[1], argv[2], argv[3], argv[4]);
    }
    catch(const std::exception& error)
    {
        std::fprintf(stderr, "%s\n", error.what());
        return 3;
    }
}
#endif
