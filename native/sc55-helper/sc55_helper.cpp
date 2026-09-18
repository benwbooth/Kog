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
#include <atomic>
#include <condition_variable>
#include <deque>
#include <iostream>
#include <limits>
#include <mutex>
#include <thread>
#include <memory>
#include <span>
#include <stdexcept>
#include <string>
#include <system_error>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#else
#include <arpa/inet.h>
#include <csignal>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>
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

void writeU32(FILE* out, uint32_t value)
{
    const std::array<uint8_t, 4> bytes = {
        static_cast<uint8_t>(value),
        static_cast<uint8_t>(value >> 8U),
        static_cast<uint8_t>(value >> 16U),
        static_cast<uint8_t>(value >> 24U),
    };
    if(std::fwrite(bytes.data(), 1, bytes.size(), out) != bytes.size())
        throw std::runtime_error("writing SC-55 helper output failed");
}

void writeU64(FILE* out, uint64_t value)
{
    writeU32(out, static_cast<uint32_t>(value));
    writeU32(out, static_cast<uint32_t>(value >> 32U));
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
    FILE* out = stdout;
    // Superseded when the requested epoch advances: a queued newer job
    // must not wait for this render to finish.
    const std::atomic<uint64_t>* epoch = nullptr;
    uint64_t expectedEpoch = 0;

    bool cancelled() const
    {
        return writeFailed || (epoch != nullptr && epoch->load() != expectedEpoch);
    }
};

void receiveSample(void* context, const AudioFrame<int32_t>& input)
{
    auto& state = *static_cast<OutputState*>(context);
    if(state.frame >= state.totalFrames || state.cancelled()) return;
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
            std::fwrite(bytes.data(), 1, bytes.size(), state.out) != bytes.size();
    }
    ++state.frame;
}

void writeHeader(FILE* out,
                 uint32_t sampleRate,
                 uint64_t totalFrames,
                 uint64_t startFrame,
                 const std::string& model)
{
    if(std::fwrite(RESPONSE_MAGIC.data(), 1, RESPONSE_MAGIC.size(), out) !=
       RESPONSE_MAGIC.size())
        throw std::runtime_error("writing SC-55 helper header failed");
    writeU32(out, PROTOCOL_VERSION);
    writeU32(out, sampleRate);
    writeU32(out, CHANNELS);
    writeU64(out, totalFrames);
    writeU64(out, startFrame);
    if(model.size() > std::numeric_limits<uint32_t>::max())
        throw std::runtime_error("SC-55 model name exceeds protocol limit");
    writeU32(out, static_cast<uint32_t>(model.size()));
    if(std::fwrite(model.data(), 1, model.size(), out) != model.size())
        throw std::runtime_error("writing SC-55 helper model failed");
    std::fflush(out);
}

struct BootedEmulator
{
    std::unique_ptr<Emulator> emulator;
    std::string model;
    uint32_t sampleRate = 0;
};

// Post-reset settle steps between songs on a live emulator: a GS reset
// silences and reinitializes the synth, so only a short drain is needed
// (tens of milliseconds emulated) rather than the full cold-boot below.
constexpr uint32_t POST_RESET_SETTLE_STEPS = 48000U;

BootedEmulator bootEmulator(const fs::path& romDirectory,
                            std::string_view requestedRomset)
{
    if(!fs::is_directory(romDirectory))
        throw std::runtime_error("SC-55 ROM path is not a directory");

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

    auto emulator = std::make_unique<Emulator>();
    if(!emulator->Init({.lcd_backend = nullptr, .nvram_filename = {}}))
        throw std::runtime_error("initializing Nuked SC-55 failed");
    if(!emulator->LoadRoms(loaded.romset, romsetInfo))
        throw std::runtime_error("installing the detected SC-55 ROM set failed");
    romsetInfo.PurgeRomData();
    emulator->Reset();
    emulator->PostSystemReset(EMU_SystemReset::GS_RESET);
    for(uint32_t step = 0; step < 24'000'000U; ++step) emulator->Step();

    const uint32_t sampleRate = PCM_GetOutputFrequency(emulator->GetPCM());
    if(sampleRate < 8'000U || sampleRate > 192'000U)
        throw std::runtime_error("Nuked SC-55 reported an invalid sample rate");
    return BootedEmulator {std::move(emulator), RomsetName(loaded.romset), sampleRate};
}

void renderJob(BootedEmulator& booted,
               const Schedule& schedule,
               uint64_t startFrame,
               FILE* out,
               const std::atomic<uint64_t>* epoch,
               uint64_t expectedEpoch)
{
    Emulator& emulator = *booted.emulator;
    const uint64_t totalFrames = framesForDuration(schedule.totalNs, booted.sampleRate);
    if(startFrame > totalFrames)
        throw std::runtime_error("SC-55 seek frame exceeds track duration");
    writeHeader(out,
                booted.sampleRate,
                totalFrames,
                startFrame,
                booted.model);

    OutputState output {.frame = 0,
                        .startFrame = startFrame,
                        .totalFrames = totalFrames,
                        .writeFailed = false,
                        .out = out,
                        .epoch = epoch,
                        .expectedEpoch = expectedEpoch};
    emulator.SetSampleCallback(receiveSample, &output);
    const uint64_t nanosecondsPerStep = emulator.GetMCU().is_mk1 ? 600U : 500U;
    uint64_t simulatedNs = 0;
    for(const Event& event : schedule.events)
    {
        while(simulatedNs < event.timestampNs && output.frame < totalFrames &&
              !output.cancelled())
        {
            emulator.Step();
            simulatedNs += nanosecondsPerStep;
        }
        if(output.frame >= totalFrames || output.cancelled()) break;
        emulator.PostMIDI(std::span<const uint8_t>(event.bytes));
    }
    while(output.frame < totalFrames && !output.cancelled()) emulator.Step();
    if(output.writeFailed)
        throw std::runtime_error("writing SC-55 PCM failed");
    if(epoch != nullptr && epoch->load() != expectedEpoch)
        throw std::runtime_error("SC-55 render superseded by a newer job");
}

void run(const fs::path& schedulePath,
         const fs::path& romDirectory,
         uint64_t startFrame,
         std::string_view requestedRomset)
{
    if(!fs::is_directory(romDirectory))
        throw std::runtime_error("SC-55 ROM path is not a directory");
    const Schedule schedule = readSchedule(schedulePath);
    BootedEmulator booted = bootEmulator(romDirectory, requestedRomset);
    renderJob(booted, schedule, startFrame, stdout, nullptr, 0);
    if(std::fflush(stdout) != 0)
        throw std::runtime_error("flushing SC-55 PCM failed");
}

} // namespace

// ---- persistent server (protocol 2): boot once, then render one job
// per stdin line over loopback TCP so each job has clean EOF-delimited
// framing. A new JOB line supersedes the in-flight render: writes to the
// abandoned socket fail fast and the loop picks the new job up.

#ifdef _WIN32
using SocketHandle = SOCKET;
constexpr SocketHandle INVALID_SOCK = INVALID_SOCKET;
#else
using SocketHandle = int;
constexpr SocketHandle INVALID_SOCK = -1;
#endif

void closeSocket(SocketHandle sock)
{
#ifdef _WIN32
    if(sock != INVALID_SOCK) ::closesocket(sock);
#else
    if(sock != INVALID_SOCK) ::close(sock);
#endif
}

struct SocketGuard
{
    SocketHandle sock = INVALID_SOCK;
    explicit SocketGuard(SocketHandle sock) : sock(sock) {}
    SocketGuard(const SocketGuard&) = delete;
    SocketGuard& operator=(const SocketGuard&) = delete;
    SocketGuard(SocketGuard&& other) noexcept : sock(other.sock) { other.sock = INVALID_SOCK; }
    SocketGuard& operator=(SocketGuard&& other) noexcept
    {
        if(this != &other)
        {
            closeSocket(sock);
            sock = other.sock;
            other.sock = INVALID_SOCK;
        }
        return *this;
    }
    ~SocketGuard() { closeSocket(sock); }
};

uint16_t parsePort(const std::string& text)
{
    unsigned long port = 0;
    const auto [end, code] = std::from_chars(text.data(), text.data() + text.size(), port);
    if(code != std::errc() || end != text.data() + text.size() || port == 0 || port > 65535)
        throw std::runtime_error("SC-55 server job has an invalid TCP port");
    return static_cast<uint16_t>(port);
}

SocketGuard connectLoopback(uint16_t port)
{
    SocketHandle sock = ::socket(AF_INET, SOCK_STREAM, 0);
    if(sock == INVALID_SOCK)
        throw std::runtime_error("SC-55 server could not create a loopback socket");
    SocketGuard guard(sock);
    sockaddr_in address {};
    address.sin_family = AF_INET;
    address.sin_port = htons(port);
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if(::connect(sock, reinterpret_cast<sockaddr*>(&address), sizeof(address)) != 0)
        throw std::runtime_error("SC-55 server could not reach its render target");
    return guard;
}

FILE* fdopenSocket(SocketHandle sock)
{
#ifdef _WIN32
    const int fd = ::_open_osfhandle(static_cast<intptr_t>(sock), 0);
    if(fd == -1)
        throw std::runtime_error("SC-55 server could not wrap its render target");
    FILE* out = ::_fdopen(fd, "wb");
#else
    FILE* out = ::fdopen(sock, "w");
#endif
    if(out == nullptr)
        throw std::runtime_error("SC-55 server could not wrap its render target");
    return out;
}

struct ServerJob
{
    std::string id;
    uint64_t startFrame = 0;
    uint16_t port = 0;
    fs::path schedulePath;
};

ServerJob parseJobLine(std::string line)
{
    // JOB\t<id>\t<start-frame>\t<tcp-port>\t<schedule-path>
    // The path runs to end of line so spaces are fine; tabs and
    // newlines in paths are rejected on the Rust side.
    if(!line.empty() && line.back() == '\r') line.pop_back();
    std::vector<std::string> fields;
    size_t begin = 0;
    for(size_t tab = 0; tab < 4; ++tab)
    {
        const size_t at = line.find('\t', begin);
        if(at == std::string::npos)
            throw std::runtime_error("SC-55 server job line is malformed");
        fields.push_back(line.substr(begin, at - begin));
        begin = at + 1;
    }
    fields.push_back(line.substr(begin));
    if(fields[0] != "JOB" || fields[1].empty() || fields[4].empty())
        throw std::runtime_error("SC-55 server job line is malformed");
    ServerJob job;
    job.id = fields[1];
    job.startFrame = parseUnsigned(fields[2].c_str(), "start frame");
    job.port = parsePort(fields[3]);
    job.schedulePath = fs::path(fields[4]);
    return job;
}

void runServer(const fs::path& romDirectory, std::string_view requestedRomset)
{
#ifdef _WIN32
    WSADATA winsock {};
    if(::WSAStartup(MAKEWORD(2, 2), &winsock) != 0)
        throw std::runtime_error("SC-55 server could not start networking");
#else
    // Abandoned render targets must fail writes instead of killing us.
    std::signal(SIGPIPE, SIG_IGN);
#endif
    BootedEmulator booted = bootEmulator(romDirectory, requestedRomset);
    std::fprintf(stderr, "READY\n");
    std::fflush(stderr);

    // A dedicate reader keeps consuming job lines while a render runs, so a
    // newer request supersedes the current one instead of queueing behind a
    // full track. The render loop watches `epoch` and abandons immediately.
    std::mutex queueMutex;
    std::condition_variable queueCv;
    std::deque<std::string> pendingLines;
    std::atomic<uint64_t> epoch {0};
    std::atomic<bool> stdinClosed {false};
    std::thread reader([&] {
        std::string line;
        while(std::getline(std::cin, line))
        {
            {
                std::lock_guard<std::mutex> lock(queueMutex);
                pendingLines.push_back(std::move(line));
            }
            epoch.fetch_add(1, std::memory_order_relaxed);
            queueCv.notify_one();
        }
        stdinClosed.store(true, std::memory_order_release);
        queueCv.notify_one();
    });

    while(true)
    {
        std::string line;
        {
            std::unique_lock<std::mutex> lock(queueMutex);
            queueCv.wait(lock, [&] {
                return !pendingLines.empty() || stdinClosed.load(std::memory_order_acquire);
            });
            if(pendingLines.empty())
                break; // stdin closed and nothing left to render
            // Only the newest request matters: older ones were superseded.
            line = std::move(pendingLines.back());
            pendingLines.clear();
        }
        try
        {
            const ServerJob job = parseJobLine(line);
            const Schedule schedule = readSchedule(job.schedulePath);
            const uint64_t expectedEpoch = epoch.load(std::memory_order_relaxed);
            booted.emulator->PostSystemReset(EMU_SystemReset::GS_RESET);
            for(uint32_t step = 0; step < POST_RESET_SETTLE_STEPS; ++step)
                booted.emulator->Step();
            SocketGuard sock = connectLoopback(job.port);
            FILE* out = fdopenSocket(sock.sock);
            // The guard must not close the socket out from under stdio.
            sock.sock = INVALID_SOCK;
            try
            {
                renderJob(booted, schedule, job.startFrame, out, &epoch, expectedEpoch);
            }
            catch(...)
            {
                std::fclose(out);
                throw;
            }
            if(std::fclose(out) != 0)
                throw std::runtime_error("closing SC-55 render target failed");
        }
        catch(const std::exception& error)
        {
            const std::string message = error.what();
            // Superseding a render is normal operation, not a failure.
            if(message.find("superseded") == std::string::npos)
            {
                std::fprintf(stderr, "JOB ERROR %s\n", message.c_str());
                std::fflush(stderr);
            }
        }
    }
    reader.join();
#ifdef _WIN32
    ::WSACleanup();
#endif
}

int main(int argc, char** argv)
{
    try
    {
        if(argc == 2 && std::strcmp(argv[1], "--version") == 0)
        {
            std::puts("kog-sc55-helper protocol 2; Nuked SC-55 0.6.1 (50dcdde)");
            return 0;
        }
        if((argc == 3 || argc == 4) && std::strcmp(argv[1], "--server") == 0)
        {
            runServer(argv[2], argc == 4 ? argv[3] : "");
            return 0;
        }
        if(argc < 4 || argc > 5)
            throw std::runtime_error(
                "usage: kog-sc55-helper <schedule> <ROM-directory> <start-frame> [ROM-set]\n"
                "   or: kog-sc55-helper --server <ROM-directory> [ROM-set]");
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
