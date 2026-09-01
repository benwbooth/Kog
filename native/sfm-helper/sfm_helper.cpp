/*
 * Kog SFM helper process.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-2.0-only
 *
 * Playback is provided by the portable GME SFM/higan core imported verbatim
 * from Cog. This adapter bounds untrusted input, restores a selected frame,
 * and streams a versioned PCM protocol; it contains no Objective-C code.
 */

#include <algorithm>
#include <array>
#include <charconv>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <limits>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

#include "Bml_Parser.h"
#include "Data_Reader.h"
#include "Spc_Sfm.h"

namespace fs = std::filesystem;

namespace
{
constexpr uint64_t MAX_FILE_BYTES = 256ULL * 1024ULL * 1024ULL;
constexpr uint32_t MAX_METADATA_BYTES = 4U * 1024U * 1024U;
constexpr uint64_t MAX_DURATION_MILLISECONDS = 12ULL * 60ULL * 60ULL * 1000ULL;
constexpr uint32_t SAMPLE_RATE = 32000U;
constexpr uint32_t CHANNELS = 2U;
constexpr uint64_t DEFAULT_LENGTH_MILLISECONDS = 150000U;
constexpr uint64_t DEFAULT_FADE_MILLISECONDS = 8000U;
constexpr size_t RAM_BYTES = 65536U;
constexpr size_t DSP_REGISTER_BYTES = 128U;
constexpr std::array<uint8_t, 8> MAGIC = {'K', 'O', 'G', 'S', 'F', 'M', '1', 0};

using Bytes = std::vector<uint8_t>;

uint32_t readU32(const uint8_t* bytes)
{
    return static_cast<uint32_t>(bytes[0]) |
           (static_cast<uint32_t>(bytes[1]) << 8U) |
           (static_cast<uint32_t>(bytes[2]) << 16U) |
           (static_cast<uint32_t>(bytes[3]) << 24U);
}

void writeU32(uint32_t value)
{
    const std::array<uint8_t, 4> bytes = {
        static_cast<uint8_t>(value),
        static_cast<uint8_t>(value >> 8U),
        static_cast<uint8_t>(value >> 16U),
        static_cast<uint8_t>(value >> 24U),
    };
    if(std::fwrite(bytes.data(), 1, bytes.size(), stdout) != bytes.size())
        throw std::runtime_error("writing SFM helper output failed");
}

void writeU64(uint64_t value)
{
    writeU32(static_cast<uint32_t>(value));
    writeU32(static_cast<uint32_t>(value >> 32U));
}

uint64_t parseUnsigned(const char* text, const char* label)
{
    uint64_t value = 0;
    const char* end = text + std::strlen(text);
    const auto result = std::from_chars(text, end, value);
    if(result.ec != std::errc {} || result.ptr != end)
        throw std::runtime_error(std::string("invalid ") + label);
    return value;
}

Bytes readFile(const fs::path& path)
{
    std::error_code error;
    const uint64_t length = fs::file_size(path, error);
    if(error) throw std::runtime_error("cannot inspect SFM file: " + error.message());
    if(length < 8U + RAM_BYTES + DSP_REGISTER_BYTES || length > MAX_FILE_BYTES)
        throw std::runtime_error("SFM file is truncated or exceeds Kog's 256 MiB limit");
    Bytes bytes(static_cast<size_t>(length));
    std::ifstream stream(path, std::ios::binary);
    if(!stream || !stream.read(reinterpret_cast<char*>(bytes.data()),
                               static_cast<std::streamsize>(bytes.size())))
        throw std::runtime_error("cannot read SFM file");
    return bytes;
}

std::optional<int64_t> metadataInteger(const Bml_Parser& metadata,
                                       const std::string& path)
{
    const char* raw = metadata.enumValue(path);
    if(!raw) return std::nullopt;
    std::string_view value(raw);
    while(!value.empty() && static_cast<unsigned char>(value.front()) <= 0x20U)
        value.remove_prefix(1);
    while(!value.empty() && static_cast<unsigned char>(value.back()) <= 0x20U)
        value.remove_suffix(1);
    int64_t parsed = 0;
    const auto result = std::from_chars(value.data(), value.data() + value.size(), parsed);
    if(value.empty() || result.ec != std::errc {} || result.ptr != value.data() + value.size())
        throw std::runtime_error("SFM metadata field " + path + " is not an integer");
    return parsed;
}

void requireRange(const Bml_Parser& metadata,
                  const std::string& path,
                  int64_t minimum,
                  int64_t maximum)
{
    const std::optional<int64_t> value = metadataInteger(metadata, path);
    if(value && (*value < minimum || *value > maximum))
        throw std::runtime_error("SFM metadata field " + path + " is outside its safe range");
}

void validateSfm(const Bytes& bytes)
{
    if(std::memcmp(bytes.data(), "SFM1", 4) != 0)
        throw std::runtime_error("file does not contain an SFM1 header");
    const uint32_t metadataBytes = readU32(bytes.data() + 4);
    if(metadataBytes > MAX_METADATA_BYTES)
        throw std::runtime_error("SFM metadata exceeds Kog's 4 MiB limit");
    const uint64_t stateOffset = 8ULL + metadataBytes;
    const uint64_t logOffset = stateOffset + RAM_BYTES + DSP_REGISTER_BYTES;
    if(logOffset > bytes.size())
        throw std::runtime_error("SFM state is truncated");

    Bml_Parser metadata;
    metadata.parseDocument(reinterpret_cast<const char*>(bytes.data() + 8), metadataBytes);
    requireRange(metadata, "timing:loopstart", 0,
                 static_cast<int64_t>(bytes.size() - logOffset));
    requireRange(metadata, "dsp:echohistaddr", 0, 7);
    for(int voice = 0; voice < 8; ++voice)
    {
        const std::string base = "dsp:voice[" + std::to_string(voice) + "]:";
        requireRange(metadata, base + "brrhistaddr", 0, 11);
        requireRange(metadata, base + "vidx", 0, 118);
        requireRange(metadata, base + "envmode", 0, 3);
    }
}

uint64_t checkedMilliseconds(long value, uint64_t fallback, const char* label)
{
    const uint64_t result = value >= 0 ? static_cast<uint64_t>(value) : fallback;
    if(result > MAX_DURATION_MILLISECONDS)
        throw std::runtime_error(std::string("SFM ") + label + " exceeds Kog's 12-hour limit");
    return result;
}

uint64_t framesForMilliseconds(uint64_t milliseconds)
{
    if(milliseconds > std::numeric_limits<uint64_t>::max() / SAMPLE_RATE)
        throw std::runtime_error("SFM duration overflows the frame counter");
    return (milliseconds * SAMPLE_RATE + 999U) / 1000U;
}

void writeString(const char* text)
{
    const std::string_view value(text ? text : "");
    if(value.size() > 65535U)
        throw std::runtime_error("SFM metadata string exceeds Kog's limit");
    writeU32(static_cast<uint32_t>(value.size()));
    if(!value.empty() && std::fwrite(value.data(), 1, value.size(), stdout) != value.size())
        throw std::runtime_error("writing SFM metadata failed");
}

void checkGme(const char* error, const char* action)
{
    if(error) throw std::runtime_error(std::string(action) + ": " + error);
}

int run(int argc, char** argv)
{
    if(argc != 3)
        throw std::runtime_error("usage: kog-sfm-helper <file> <start-frame>");
#ifdef _WIN32
    _setmode(_fileno(stdout), _O_BINARY);
#endif
    const uint64_t requestedStartFrame = parseUnsigned(argv[2], "SFM start frame");
    Bytes bytes = readFile(fs::u8path(argv[1]));
    validateSfm(bytes);

    Sfm_Emu emulator;
    checkGme(emulator.set_sample_rate(SAMPLE_RATE), "setting the SFM sample rate");
    // Fixed-track GME types must load through Data_Reader so Gme_File records
    // the track boundaries used again by Music_Emu::start_track_. This mirrors
    // Cog's gme_load_custom path; calling load_mem() directly skips that setup.
    Mem_File_Reader reader(bytes.data(), static_cast<long>(bytes.size()));
    checkGme(emulator.load(reader), "loading the SFM file");

    track_info_t info {};
    checkGme(emulator.track_info(&info, 0), "reading SFM metadata");
    uint64_t mainMilliseconds = DEFAULT_LENGTH_MILLISECONDS;
    if(info.length > 0)
        mainMilliseconds = checkedMilliseconds(info.length, DEFAULT_LENGTH_MILLISECONDS,
                                               "length");
    else if(info.loop_length > 0)
    {
        const uint64_t intro = checkedMilliseconds(info.intro_length, 0, "intro length");
        const uint64_t loop = checkedMilliseconds(info.loop_length, 0, "loop length");
        if(loop > (MAX_DURATION_MILLISECONDS - intro) / 2U)
            throw std::runtime_error("SFM loop duration exceeds Kog's limit");
        mainMilliseconds = intro + loop * 2U;
    }
    const uint64_t fadeMilliseconds =
        checkedMilliseconds(info.fade_length, DEFAULT_FADE_MILLISECONDS, "fade length");
    if(mainMilliseconds > MAX_DURATION_MILLISECONDS - fadeMilliseconds)
        throw std::runtime_error("SFM total duration exceeds Kog's limit");
    const uint64_t totalMilliseconds = mainMilliseconds + fadeMilliseconds;
    const uint64_t mainFrames = framesForMilliseconds(mainMilliseconds);
    const uint64_t totalFrames = framesForMilliseconds(totalMilliseconds);
    const uint64_t startFrame = std::min(requestedStartFrame, totalFrames);

    checkGme(emulator.start_track(0), "starting SFM playback");
    emulator.set_fade(static_cast<long>(mainMilliseconds), static_cast<long>(fadeMilliseconds));
    std::array<int16_t, 4096U * CHANNELS> pcm {};
    uint64_t skipped = 0;
    while(skipped < startFrame)
    {
        const uint64_t frames = std::min<uint64_t>(4096U, startFrame - skipped);
        checkGme(emulator.skip(static_cast<long>(frames * CHANNELS)), "seeking SFM playback");
        skipped += frames;
    }

    if(std::fwrite(MAGIC.data(), 1, MAGIC.size(), stdout) != MAGIC.size())
        throw std::runtime_error("writing SFM helper header failed");
    writeU32(1U);
    writeU32(SAMPLE_RATE);
    writeU32(CHANNELS);
    writeU64(totalFrames);
    writeU64(mainFrames);
    writeString(info.system);
    writeString(info.song);
    writeString(info.game);
    writeString(info.author);
    writeString(info.copyright);
    writeString(info.date);

    uint64_t rendered = startFrame;
    while(rendered < totalFrames)
    {
        const size_t frames = static_cast<size_t>(std::min<uint64_t>(4096U, totalFrames - rendered));
        checkGme(emulator.play(static_cast<long>(frames * CHANNELS), pcm.data()),
                 "rendering SFM audio");
        const size_t samples = frames * CHANNELS;
        if(std::fwrite(pcm.data(), sizeof(int16_t), samples, stdout) != samples)
            throw std::runtime_error("writing SFM PCM failed");
        rendered += frames;
    }
    return 0;
}
}

int main(int argc, char** argv)
{
    try
    {
        return run(argc, argv);
    }
    catch(const std::exception& error)
    {
        std::fprintf(stderr, "kog-sfm-helper: %s\n", error.what());
        return 1;
    }
}
