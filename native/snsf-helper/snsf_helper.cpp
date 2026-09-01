/*
 * Kog SNSF helper process.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: LicenseRef-Snes9x
 *
 * This adapter is distributed with the separately licensed libsnsf9x helper.
 * psflib safely assembles xSF dependencies; libsnsf9x supplies SNSF playback.
 * No Cog Objective-C or Objective-C++ decoder source is translated here.
 */

#include <algorithm>
#include <array>
#include <cerrno>
#include <climits>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <limits>
#include <map>
#include <memory>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#endif

#include <zlib.h>

#include "psflib.h"
#include "pversion.h"
#include "xsfc/xsfdrv.h"

extern "C" IXSFDRV* XSFSetup(LPFNGETLIB_XSFDRV callback, void* context);

namespace fs = std::filesystem;

namespace
{
constexpr uint64_t MAX_FILE_BYTES = 64ULL * 1024ULL * 1024ULL;
constexpr uint64_t MAX_TOTAL_FILE_BYTES = 256ULL * 1024ULL * 1024ULL;
constexpr size_t MAX_FILES = 128U;
constexpr uint64_t MAX_ROM_BYTES = 8ULL * 1024ULL * 1024ULL;
constexpr size_t SRAM_BYTES = 128U * 1024U;
constexpr uint32_t MAX_DURATION_MILLISECONDS = 24U * 60U * 60U * 1000U;
constexpr uint32_t SAMPLE_RATE = 32000U;
constexpr uint32_t CHANNELS = 2U;
constexpr uint32_t FORMAT_VERSION = 0x23U;
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

void appendU32(Bytes& bytes, uint32_t value)
{
    bytes.push_back(static_cast<uint8_t>(value));
    bytes.push_back(static_cast<uint8_t>(value >> 8U));
    bytes.push_back(static_cast<uint8_t>(value >> 16U));
    bytes.push_back(static_cast<uint8_t>(value >> 24U));
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

struct SourceContext
{
    fs::path rootDirectory;
    std::set<fs::path> files;
    uint64_t totalBytes = 0;
    std::string error;
};

struct InputFile
{
    std::FILE* stream = nullptr;
};

bool pathIsWithin(const fs::path& path, const fs::path& directory)
{
    const fs::path relative = path.lexically_relative(directory);
    if(relative.empty() || relative.is_absolute()) return false;
    const auto first = relative.begin();
    return first != relative.end() && *first != "..";
}

void* sourceOpen(void* contextPointer, const char* pathText)
{
    auto& context = *static_cast<SourceContext*>(contextPointer);
    if(pathText == nullptr) return nullptr;
    std::error_code error;
    const fs::path path = fs::canonical(fs::u8path(pathText), error);
    if(error || !pathIsWithin(path, context.rootDirectory))
    {
        context.error = "SNSF dependency escapes the root file's directory";
        return nullptr;
    }
#ifdef _WIN32
    std::FILE* stream = _wfopen(path.c_str(), L"rb");
#else
    std::FILE* stream = std::fopen(path.c_str(), "rb");
#endif
    if(stream == nullptr)
    {
        context.error = "cannot open SNSF dependency " + displayPath(path);
        return nullptr;
    }
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
        context.error = "SNSF dependency exceeds Kog's per-file limit";
        return nullptr;
    }
    if(context.files.insert(path).second)
    {
        if(context.files.size() > MAX_FILES ||
           static_cast<uint64_t>(length) > MAX_TOTAL_FILE_BYTES - context.totalBytes)
        {
            std::fclose(stream);
            context.error = "SNSF dependency set exceeds Kog's limit";
            return nullptr;
        }
        context.totalBytes += static_cast<uint64_t>(length);
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

struct LoadState
{
    Bytes rom;
    Bytes sram;
    Tags tags;
    bool first = false;
    uint32_t base = 0;
    std::string error;
};

void mapProgram(LoadState& state, const uint8_t* data, size_t dataSize)
{
    if(data == nullptr || dataSize < 8)
        throw std::runtime_error("SNSF mapping is shorter than its header");
    uint64_t offset = readU32(data);
    const uint64_t size = readU32(data + 4);
    if(size > dataSize - 8)
        throw std::runtime_error("SNSF mapping payload exceeds its decompressed section");
    if(!state.first)
    {
        state.first = true;
        state.base = static_cast<uint32_t>(offset);
    }
    else
    {
        offset += state.base;
        if(offset > std::numeric_limits<uint32_t>::max())
            throw std::runtime_error("SNSF relative mapping offset overflowed");
    }
    offset &= 0x1FFFFFFFU;
    if(offset > MAX_ROM_BYTES || size > MAX_ROM_BYTES - offset)
        throw std::runtime_error("SNSF ROM mapping exceeds the libsnsf9x core limit");
    const size_t required = static_cast<size_t>(offset + size);
    if(state.rom.size() < required) state.rom.resize(required, 0);
    std::copy_n(data + 8, static_cast<size_t>(size),
                state.rom.begin() + static_cast<ptrdiff_t>(offset));
}

void mapReserved(LoadState& state, const uint8_t* reserved, size_t reservedSize)
{
    size_t position = 0;
    while(position < reservedSize)
    {
        if(!checkedRange(position, 8, reservedSize))
            throw std::runtime_error("SNSF reserved block has a truncated header");
        const uint32_t type = readU32(reserved + position);
        const uint32_t size = readU32(reserved + position + 4);
        position += 8;
        if(!checkedRange(position, size, reservedSize))
            throw std::runtime_error("SNSF reserved record exceeds its section");
        if(type == 0)
        {
            if(size < 4)
                throw std::runtime_error("SNSF SRAM record is shorter than its offset");
            const uint64_t offset = readU32(reserved + position);
            const uint64_t payload = static_cast<uint64_t>(size) - 4;
            if(offset > SRAM_BYTES || payload > SRAM_BYTES - offset)
                throw std::runtime_error("SNSF SRAM mapping exceeds 128 KiB");
            if(state.sram.empty()) state.sram.resize(SRAM_BYTES, 0xFF);
            std::copy_n(reserved + position + 4, static_cast<size_t>(payload),
                        state.sram.begin() + static_cast<ptrdiff_t>(offset));
        }
        position += size;
    }
}

int loadSections(void* context, const uint8_t* executable, size_t executableSize,
                 const uint8_t* reserved, size_t reservedSize)
{
    auto& state = *static_cast<LoadState*>(context);
    try
    {
        if(reservedSize != 0) mapReserved(state, reserved, reservedSize);
        if(executableSize != 0) mapProgram(state, executable, executableSize);
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
    if(key.size() > 128 || std::strlen(value) > 16U * 1024U || state.tags.size() >= 128U)
    {
        state.error = "SNSF metadata exceeds Kog's limit";
        return -1;
    }
    state.tags[key] = value;
    return 0;
}

void loadStatus(void* context, const char* message)
{
    auto& state = *static_cast<LoadState*>(context);
    if(state.error.empty() && message != nullptr) state.error = message;
}

Bytes compressProgram(const Bytes& rom)
{
    if(rom.empty() || rom.size() > std::numeric_limits<uint32_t>::max())
        throw std::runtime_error("SNSF dependency chain did not produce a ROM");
    Bytes mapped;
    mapped.reserve(8 + rom.size());
    appendU32(mapped, 0);
    appendU32(mapped, static_cast<uint32_t>(rom.size()));
    mapped.insert(mapped.end(), rom.begin(), rom.end());
    uLongf compressedSize = compressBound(static_cast<uLong>(mapped.size()));
    Bytes compressed(static_cast<size_t>(compressedSize));
    if(compress2(compressed.data(), &compressedSize, mapped.data(),
                 static_cast<uLong>(mapped.size()), Z_BEST_SPEED) != Z_OK)
        throw std::runtime_error("cannot create sanitized SNSF program image");
    compressed.resize(static_cast<size_t>(compressedSize));
    return compressed;
}

Bytes sanitizedSnsf(const LoadState& state)
{
    const Bytes compressed = compressProgram(state.rom);
    Bytes reserved;
    if(!state.sram.empty())
    {
        appendU32(reserved, 0);
        appendU32(reserved, static_cast<uint32_t>(4 + state.sram.size()));
        appendU32(reserved, 0);
        reserved.insert(reserved.end(), state.sram.begin(), state.sram.end());
    }
    Bytes output;
    output.reserve(16 + reserved.size() + compressed.size());
    output.insert(output.end(), {'P', 'S', 'F', 0x23});
    appendU32(output, static_cast<uint32_t>(reserved.size()));
    appendU32(output, static_cast<uint32_t>(compressed.size()));
    appendU32(output, static_cast<uint32_t>(crc32(0, compressed.data(),
                                                   static_cast<uInt>(compressed.size()))));
    output.insert(output.end(), reserved.begin(), reserved.end());
    output.insert(output.end(), compressed.begin(), compressed.end());
    return output;
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
            throw std::runtime_error("writing SNSF PCM failed");
        frame += chunkFrames;
    }
}

int rejectLibrary(void*, char*, void**, uint32_t*)
{
    return -1;
}

class DriverSession
{
public:
    explicit DriverSession(IXSFDRV* driver) : driver_(driver) {}
    ~DriverSession()
    {
        if(started_) driver_->Term();
    }
    void start(Bytes& image)
    {
        if(driver_->Start(image.data(), static_cast<uint32_t>(image.size())) != 0)
            throw std::runtime_error("libsnsf9x rejected the sanitized SNSF image");
        started_ = true;
    }

private:
    IXSFDRV* driver_;
    bool started_ = false;
};

int runHelper(const std::string& pathText, const char* startText, const char* defaultLengthText,
              const char* defaultFadeText)
{
    uint64_t startFrame = parseU64(startText, "SNSF start frame");
    const uint64_t defaultLength = parseU64(defaultLengthText, "SNSF default length");
    const uint64_t defaultFade = parseU64(defaultFadeText, "SNSF default fade");
    if(defaultLength == 0 || defaultLength > MAX_DURATION_MILLISECONDS ||
       defaultFade > MAX_DURATION_MILLISECONDS)
        throw std::runtime_error("SNSF default duration exceeds Kog's limit");

    std::error_code pathError;
    const fs::path canonical = fs::canonical(fs::u8path(pathText), pathError);
    if(pathError) throw std::runtime_error("cannot resolve SNSF root file");
    SourceContext source {canonical.parent_path(), {}, 0, {}};
    const psf_file_callbacks callbacks = {
        "/|\\", &source, sourceOpen, sourceRead, sourceSeek, sourceClose, sourceTell};
    LoadState state;
    if(psf_load(displayPath(canonical).c_str(), &callbacks, 0x23, loadSections, &state, loadTag,
                &state, 1, loadStatus, &state) != 0x23)
        throw std::runtime_error(!source.error.empty() ? source.error :
                                (!state.error.empty() ? state.error :
                                 "psflib rejected the SNSF dependency chain"));
    if(psf_load(displayPath(canonical).c_str(), &callbacks, 0x23, nullptr, nullptr, loadTag,
                &state, 0, nullptr, nullptr) != 0x23)
        throw std::runtime_error("psflib could not read the outer SNSF metadata");
    // The Snes9x cartridge scorer reads the complete 32 KiB LoROM header
    // region even when a malformed mapping supplies only a few bytes.
    if(state.rom.size() < 0x8000U)
        throw std::runtime_error("SNSF dependency chain did not produce a complete ROM header");

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
        throw std::runtime_error("invalid SNSF duration metadata");
    const uint64_t totalFrames = mainFrames + fadeFrames;
    startFrame = std::min(startFrame, totalFrames);

    Bytes image = sanitizedSnsf(state);
    IXSFDRV* driver = XSFSetup(rejectLibrary, nullptr);
    if(driver == nullptr || driver->dwInterfaceVersion < 3 || driver->SetExtendParamVoid == nullptr)
        throw std::runtime_error("libsnsf9x does not provide the required interface");
    const int interpolation = 0; // Gaussian, matching Cog's SNSF setting.
    const int resampler = 1; // libsnsf9x's default Hermite rate conversion.
    const uint32_t sampleRate = SAMPLE_RATE;
    driver->SetExtendParamVoid(XSFDRIVER_EXTENDPARAM_INTERPOLATION, &interpolation);
    driver->SetExtendParamVoid(XSFDRIVER_EXTENDPARAM_RESAMPLER, &resampler);
    driver->SetExtendParamVoid(XSFDRIVER_EXTENDPARAM_SAMPLE_RATE, &sampleRate);
    DriverSession session(driver);
    session.start(image);

    if(!writeHeader(state.tags, mainFrames, totalFrames))
        throw std::runtime_error("writing the SNSF stream header failed");
    std::vector<int16_t> samples(2048U * CHANNELS);
    uint64_t sourceFrame = 0;
    while(sourceFrame < totalFrames)
    {
        const size_t frames = static_cast<size_t>(
            std::min<uint64_t>(samples.size() / CHANNELS, totalFrames - sourceFrame));
        driver->Gen(samples.data(), static_cast<uint32_t>(frames));
        const uint64_t begin = std::max(sourceFrame, startFrame);
        const uint64_t end = sourceFrame + frames;
        if(begin < end)
        {
            const size_t skip = static_cast<size_t>(begin - sourceFrame);
            writeSamples(samples.data() + skip * CHANNELS, static_cast<size_t>(end - begin));
        }
        sourceFrame = end;
    }
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
                     "usage: kog-snsf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
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
                     "usage: kog-snsf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
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
