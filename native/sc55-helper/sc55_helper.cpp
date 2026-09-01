/*
 * Kog Nuked SC-55 helper process.
 * Copyright (C) 2026 Kog contributors.
 * LicenseRef-Nuked-SC55
 *
 * This adapter is distributed under the same original non-commercial MAME
 * terms as the separately pinned Nuked SC-55 backend. See that checkout's
 * LICENSE file and Kog's THIRD_PARTY_NOTICES.md.
 */

#include <algorithm>
#include <array>
#include <charconv>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <limits>
#include <span>
#include <stdexcept>
#include <string>
#include <system_error>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

#include "audio.h"
#include "emu.h"
#include "pcm.h"
#include "rom_loader.h"

namespace fs = std::filesystem;

namespace
{
constexpr uint64_t MAX_SCHEDULE_BYTES = 256ULL * 1024ULL * 1024ULL;
constexpr uint32_t MAX_EVENTS = 2'000'000U;
constexpr uint32_t MAX_EVENT_BYTES = 1024U * 1024U;
constexpr uint64_t MAX_DURATION_NS = 24ULL * 60ULL * 60ULL * 1'000'000'000ULL;
constexpr uint64_t NS_PER_SECOND = 1'000'000'000ULL;
constexpr uint32_t CHANNELS = 2U;
constexpr uint32_t PROTOCOL_VERSION = 1U;
constexpr std::array<uint8_t, 8> SCHEDULE_MAGIC = {'K', 'O', 'G', 'S', 'C', 'M', '1', 0};
constexpr std::array<uint8_t, 8> RESPONSE_MAGIC = {'K', 'O', 'G', 'S', 'C', '5', '5', '1'};

using Bytes = std::vector<uint8_t>;

uint32_t readU32(const uint8_t* bytes)
{
    return static_cast<uint32_t>(bytes[0]) |
           (static_cast<uint32_t>(bytes[1]) << 8U) |
           (static_cast<uint32_t>(bytes[2]) << 16U) |
           (static_cast<uint32_t>(bytes[3]) << 24U);
}

uint64_t readU64(const uint8_t* bytes)
{
    return static_cast<uint64_t>(readU32(bytes)) |
           (static_cast<uint64_t>(readU32(bytes + 4)) << 32U);
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
        throw std::runtime_error("writing SC-55 helper output failed");
}

void writeU64(uint64_t value)
{
    writeU32(static_cast<uint32_t>(value));
    writeU32(static_cast<uint32_t>(value >> 32U));
}

struct Cursor
{
    const Bytes& bytes;
    size_t position = 0;

    const uint8_t* take(size_t count, const char* label)
    {
        if(count > bytes.size() - std::min(position, bytes.size()))
            throw std::runtime_error(std::string("truncated SC-55 schedule ") + label);
        const uint8_t* result = bytes.data() + position;
        position += count;
        return result;
    }
};

struct Event
{
    uint64_t timestampNs;
    Bytes bytes;
};

struct Schedule
{
    uint64_t totalNs;
    std::vector<Event> events;
};

Bytes readFile(const fs::path& path)
{
    std::error_code error;
    const uint64_t length = fs::file_size(path, error);
    if(error) throw std::runtime_error("cannot inspect SC-55 schedule: " + error.message());
    if(length == 0 || length > MAX_SCHEDULE_BYTES)
        throw std::runtime_error("SC-55 schedule is empty or exceeds 256 MiB");
    Bytes bytes(static_cast<size_t>(length));
    std::ifstream stream(path, std::ios::binary);
    if(!stream || !stream.read(reinterpret_cast<char*>(bytes.data()),
                               static_cast<std::streamsize>(bytes.size())))
        throw std::runtime_error("cannot read SC-55 schedule");
    return bytes;
}

Schedule readSchedule(const fs::path& path)
{
    const Bytes bytes = readFile(path);
    Cursor cursor {bytes};
    const uint8_t* magic = cursor.take(SCHEDULE_MAGIC.size(), "magic");
    if(!std::equal(SCHEDULE_MAGIC.begin(), SCHEDULE_MAGIC.end(), magic))
        throw std::runtime_error("invalid SC-55 schedule magic");
    if(readU32(cursor.take(4, "version")) != PROTOCOL_VERSION)
        throw std::runtime_error("unsupported SC-55 schedule version");
    Schedule schedule;
    schedule.totalNs = readU64(cursor.take(8, "duration"));
    if(schedule.totalNs == 0 || schedule.totalNs > MAX_DURATION_NS)
        throw std::runtime_error("SC-55 schedule duration is outside Kog's limit");
    const uint32_t eventCount = readU32(cursor.take(4, "event count"));
    if(eventCount > MAX_EVENTS)
        throw std::runtime_error("SC-55 schedule has too many events");
    schedule.events.reserve(eventCount);
    uint64_t previous = 0;
    for(uint32_t index = 0; index < eventCount; ++index)
    {
        const uint64_t timestamp = readU64(cursor.take(8, "event timestamp"));
        const uint32_t length = readU32(cursor.take(4, "event length"));
        if(timestamp < previous || timestamp > schedule.totalNs || length == 0 ||
           length > MAX_EVENT_BYTES)
            throw std::runtime_error("SC-55 schedule has an invalid event");
        const uint8_t* data = cursor.take(length, "event data");
        schedule.events.push_back({timestamp, Bytes(data, data + length)});
        previous = timestamp;
    }
    if(cursor.position != bytes.size())
        throw std::runtime_error("SC-55 schedule contains trailing data");
    return schedule;
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

uint64_t framesForDuration(uint64_t nanoseconds, uint32_t sampleRate)
{
    const uint64_t whole = nanoseconds / NS_PER_SECOND;
    const uint64_t remainder = nanoseconds % NS_PER_SECOND;
    if(whole > std::numeric_limits<uint64_t>::max() / sampleRate)
        throw std::runtime_error("SC-55 frame duration overflow");
    const uint64_t fractional =
        (remainder * sampleRate + NS_PER_SECOND - 1U) / NS_PER_SECOND;
    return whole * sampleRate + fractional;
}

struct OutputState
{
    uint64_t frame = 0;
    uint64_t startFrame = 0;
    uint64_t totalFrames = 0;
    bool writeFailed = false;
};

void receiveSample(void* context, const AudioFrame<int32_t>& input)
{
    auto& state = *static_cast<OutputState*>(context);
    if(state.frame >= state.totalFrames || state.writeFailed) return;
    if(state.frame >= state.startFrame)
    {
        AudioFrame<int16_t> output;
        Normalize(input, output);
        const std::array<uint8_t, 4> bytes = {
            static_cast<uint8_t>(static_cast<uint16_t>(output.left)),
            static_cast<uint8_t>(static_cast<uint16_t>(output.left) >> 8U),
            static_cast<uint8_t>(static_cast<uint16_t>(output.right)),
            static_cast<uint8_t>(static_cast<uint16_t>(output.right) >> 8U),
        };
        state.writeFailed =
            std::fwrite(bytes.data(), 1, bytes.size(), stdout) != bytes.size();
    }
    ++state.frame;
}

void writeHeader(uint32_t sampleRate,
                 uint64_t totalFrames,
                 uint64_t startFrame,
                 const std::string& model)
{
    if(std::fwrite(RESPONSE_MAGIC.data(), 1, RESPONSE_MAGIC.size(), stdout) !=
       RESPONSE_MAGIC.size())
        throw std::runtime_error("writing SC-55 helper header failed");
    writeU32(PROTOCOL_VERSION);
    writeU32(sampleRate);
    writeU32(CHANNELS);
    writeU64(totalFrames);
    writeU64(startFrame);
    if(model.size() > std::numeric_limits<uint32_t>::max())
        throw std::runtime_error("SC-55 model name exceeds protocol limit");
    writeU32(static_cast<uint32_t>(model.size()));
    if(std::fwrite(model.data(), 1, model.size(), stdout) != model.size())
        throw std::runtime_error("writing SC-55 helper model failed");
    std::fflush(stdout);
}

void run(const fs::path& schedulePath,
         const fs::path& romDirectory,
         uint64_t startFrame,
         std::string_view requestedRomset)
{
    if(!fs::is_directory(romDirectory))
        throw std::runtime_error("SC-55 ROM path is not a directory");
    const Schedule schedule = readSchedule(schedulePath);

    AllRomsetInfo romsetInfo;
    common::LoadRomsetResult loaded;
    const common::RomOverrides overrides {};
    const common::LoadRomsetError loadError = common::LoadRomset(
        romsetInfo, romDirectory, requestedRomset, false, overrides, loaded);
    if(loadError != common::LoadRomsetError {})
    {
        common::PrintLoadRomsetDiagnostics(stderr, loadError, loaded, romsetInfo);
        throw std::runtime_error(
            std::string("loading SC-55 ROM set failed: ") + common::ToCString(loadError));
    }

    Emulator emulator;
    if(!emulator.Init({.lcd_backend = nullptr, .nvram_filename = {}}))
        throw std::runtime_error("initializing Nuked SC-55 failed");
    if(!emulator.LoadRoms(loaded.romset, romsetInfo))
        throw std::runtime_error("installing the detected SC-55 ROM set failed");
    romsetInfo.PurgeRomData();
    emulator.Reset();
    emulator.PostSystemReset(EMU_SystemReset::GS_RESET);
    for(uint32_t step = 0; step < 24'000'000U; ++step) emulator.Step();

    const uint32_t sampleRate = PCM_GetOutputFrequency(emulator.GetPCM());
    if(sampleRate < 8'000U || sampleRate > 192'000U)
        throw std::runtime_error("Nuked SC-55 reported an invalid sample rate");
    const uint64_t totalFrames = framesForDuration(schedule.totalNs, sampleRate);
    if(startFrame > totalFrames)
        throw std::runtime_error("SC-55 seek frame exceeds track duration");
    writeHeader(sampleRate,
                totalFrames,
                startFrame,
                RomsetName(loaded.romset));

    OutputState output {.frame = 0,
                        .startFrame = startFrame,
                        .totalFrames = totalFrames,
                        .writeFailed = false};
    emulator.SetSampleCallback(receiveSample, &output);
    const uint64_t nanosecondsPerStep = emulator.GetMCU().is_mk1 ? 600U : 500U;
    uint64_t simulatedNs = 0;
    for(const Event& event : schedule.events)
    {
        while(simulatedNs < event.timestampNs && output.frame < totalFrames &&
              !output.writeFailed)
        {
            emulator.Step();
            simulatedNs += nanosecondsPerStep;
        }
        if(output.frame >= totalFrames || output.writeFailed) break;
        emulator.PostMIDI(std::span<const uint8_t>(event.bytes));
    }
    while(output.frame < totalFrames && !output.writeFailed) emulator.Step();
    if(output.writeFailed)
        throw std::runtime_error("writing SC-55 PCM failed");
}
} // namespace

int main(int argc, char** argv)
{
    try
    {
        if(argc == 2 && std::strcmp(argv[1], "--version") == 0)
        {
            std::puts("kog-sc55-helper protocol 1; Nuked SC-55 0.6.1 (50dcdde)");
            return 0;
        }
        if(argc < 4 || argc > 5)
            throw std::runtime_error(
                "usage: kog-sc55-helper <schedule> <ROM-directory> <start-frame> [ROM-set]");
#ifdef _WIN32
        _setmode(_fileno(stdout), _O_BINARY);
#endif
        run(argv[1], argv[2], parseUnsigned(argv[3], "start frame"), argc == 5 ? argv[4] : "");
        return 0;
    }
    catch(const std::exception& error)
    {
        std::fprintf(stderr, "kog-sc55-helper: %s\n", error.what());
        return 1;
    }
}
