/*
 * Kog Syntrax helper process.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * The renderer is the separately pinned syntrax-c library. This adapter only
 * validates untrusted JXS structure, selects a subsong, and streams PCM; it
 * does not translate Cog's Objective-C plugin.
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
#include <memory>
#include <stdexcept>
#include <string>
#include <system_error>
#include <utility>
#include <vector>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

#include "jaytrax.h"
#include "jxs.h"

namespace fs = std::filesystem;

namespace
{
constexpr uint64_t MAX_FILE_BYTES = 256ULL * 1024ULL * 1024ULL;
constexpr int32_t MAX_PATTERNS = 4096;
constexpr int32_t MAX_SUBSONGS = 256;
constexpr int32_t MAX_INSTRUMENTS = 255;
constexpr int32_t MAX_NAME_BYTES = 1024 * 1024;
constexpr int32_t MAX_SAMPLE_BYTES = 64 * 1024 * 1024;
constexpr uint32_t SAMPLE_RATE = 44100U;
constexpr uint32_t CHANNELS = 2U;
constexpr uint32_t LOOP_COUNT = 2U;
constexpr uint64_t FADE_FRAMES = 8ULL * SAMPLE_RATE;
constexpr std::array<uint8_t, 8> MAGIC = {'K', 'O', 'G', 'J', 'X', 'S', '1', 0};

using Bytes = std::vector<uint8_t>;

uint16_t readU16(const uint8_t* bytes)
{
    return static_cast<uint16_t>(bytes[0]) |
           static_cast<uint16_t>(static_cast<uint16_t>(bytes[1]) << 8U);
}

int16_t readI16(const uint8_t* bytes)
{
    return static_cast<int16_t>(readU16(bytes));
}

uint32_t readU32(const uint8_t* bytes)
{
    return static_cast<uint32_t>(bytes[0]) |
           (static_cast<uint32_t>(bytes[1]) << 8U) |
           (static_cast<uint32_t>(bytes[2]) << 16U) |
           (static_cast<uint32_t>(bytes[3]) << 24U);
}

int32_t readI32(const uint8_t* bytes)
{
    return static_cast<int32_t>(readU32(bytes));
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
        throw std::runtime_error("writing Syntrax helper output failed");
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
            throw std::runtime_error(std::string("truncated JXS ") + label);
        const uint8_t* result = bytes.data() + position;
        position += count;
        return result;
    }
};

struct PatternNote
{
    uint8_t note;
    uint8_t instrument;
    uint8_t destination;
    uint8_t script;
};

struct InstrumentInfo
{
    uint8_t arpeggio;
};

template<typename Field>
const uint8_t* fieldAt(const uint8_t* base, Field f_JT1Subsong::* field)
{
    const auto* object = reinterpret_cast<const f_JT1Subsong*>(base);
    return reinterpret_cast<const uint8_t*>(&(object->*field));
}

bool isPowerOfTwo(int32_t value)
{
    return value > 0 && (value & (value - 1)) == 0;
}

void validateOrders(const uint8_t* subsong, int32_t patternCount, int32_t requiredStep)
{
    const size_t ordersOffset = offsetof(f_JT1Subsong, orders);
    for(size_t channel = 0; channel < J3457_CHANS_SUBSONG; ++channel)
    {
        int32_t covered = 0;
        for(size_t order = 0; order < J3457_ORDERS_SUBSONG && covered <= requiredStep; ++order)
        {
            const size_t offset = ordersOffset +
                                  (channel * J3457_ORDERS_SUBSONG + order) *
                                      sizeof(f_JT1Order);
            const int32_t pattern = readI16(subsong + offset);
            const int32_t length = readI16(subsong + offset + sizeof(int16_t));
            if(pattern < 0 || pattern >= patternCount || length <= 0 || length > J3457_ROWS_PAT)
                throw std::runtime_error("JXS order references an invalid pattern or length");
            if(covered > std::numeric_limits<int32_t>::max() - length)
                throw std::runtime_error("JXS order length overflow");
            covered += length;
        }
        if(covered <= requiredStep)
            throw std::runtime_error("JXS orders do not cover the declared song range");
    }
}

void validateSubsong(const uint8_t* bytes, int32_t patternCount)
{
    const int32_t speed = readI32(fieldAt(bytes, &f_JT1Subsong::songspd));
    const int32_t groove = readI32(fieldAt(bytes, &f_JT1Subsong::groove));
    const int32_t startPosition = readI32(fieldAt(bytes, &f_JT1Subsong::songpos));
    const int32_t startStep = readI32(fieldAt(bytes, &f_JT1Subsong::songstep));
    const int32_t endPosition = readI32(fieldAt(bytes, &f_JT1Subsong::endpos));
    const int32_t endStep = readI32(fieldAt(bytes, &f_JT1Subsong::endstep));
    const int32_t loopPosition = readI32(fieldAt(bytes, &f_JT1Subsong::looppos));
    const int32_t loopStep = readI32(fieldAt(bytes, &f_JT1Subsong::loopstep));
    const int32_t channels = readI16(fieldAt(bytes, &f_JT1Subsong::nrofchans));
    const uint32_t delay = readU16(fieldAt(bytes, &f_JT1Subsong::delaytime));
    const int32_t amplification = readI16(fieldAt(bytes, &f_JT1Subsong::amplification));
    const bool loops = readI16(fieldAt(bytes, &f_JT1Subsong::songloop)) != 0;

    if(speed <= 0 || speed > 1000 || groove < -7 || groove > 7)
        throw std::runtime_error("JXS subsong has an invalid speed or groove");
    if(channels <= 0 || channels > J3457_CHANS_SUBSONG || delay == 0 ||
       amplification < 0 || amplification > 1000)
        throw std::runtime_error("JXS subsong has invalid mixer properties");
    const auto validPosition = [](int32_t position) { return position >= 0 && position < 256; };
    const auto validStep = [](int32_t step) { return step >= 0 && step < 64; };
    if(!validPosition(startPosition) || !validPosition(endPosition) ||
       !validPosition(loopPosition) || !validStep(startStep) || !validStep(endStep) ||
       !validStep(loopStep))
        throw std::runtime_error("JXS subsong has an invalid position");
    const int32_t start = startPosition * 64 + startStep;
    const int32_t end = endPosition * 64 + endStep;
    const int32_t loop = loopPosition * 64 + loopStep;
    if(end <= start || (loops && (loop < start || loop >= end)))
        throw std::runtime_error("JXS subsong has an invalid play or loop range");
    validateOrders(bytes, patternCount, end);
}

InstrumentInfo validateInstrument(const uint8_t* bytes, int32_t instrumentCount)
{
    const auto i16 = [bytes](size_t offset) { return readI16(bytes + offset); };
    const auto i32 = [bytes](size_t offset) { return readI32(bytes + offset); };
    const int32_t waveform = i16(offsetof(f_JT1Inst, waveform));
    const int32_t wavelength = i16(offsetof(f_JT1Inst, wavelength));
    const int32_t amwave = i16(offsetof(f_JT1Inst, amwave));
    const int32_t finetune = i16(offsetof(f_JT1Inst, finetune));
    const int32_t fmwave = i16(offsetof(f_JT1Inst, fmwave));
    const int32_t arpeggio = i16(offsetof(f_JT1Inst, arpeggio));
    const int32_t panwave = i16(offsetof(f_JT1Inst, panwave));
    const int32_t sharing = i16(offsetof(f_JT1Inst, sharing));
    if(waveform < 0 || waveform >= J3457_WAVES_INST || !isPowerOfTwo(wavelength) ||
       wavelength > J3457_SAMPS_WAVE || amwave < 0 || amwave > J3457_WAVES_INST ||
       fmwave < 0 || fmwave > J3457_WAVES_INST || panwave < 0 ||
       panwave > J3457_WAVES_INST || finetune < 0 || finetune >= 16 || arpeggio < 0 ||
       arpeggio >= J3457_ARPS_SONG || sharing < 0 || sharing > instrumentCount)
        throw std::runtime_error("JXS instrument has an invalid synthesis index");

    for(size_t index = 0; index < J3457_EFF_INST; ++index)
    {
        const size_t base = offsetof(f_JT1Inst, fx) + index * sizeof(f_JT1Effect);
        const int32_t destination = i32(base + offsetof(f_JT1Effect, dsteffect));
        const int32_t source1 = i32(base + offsetof(f_JT1Effect, srceffect1));
        const int32_t source2 = i32(base + offsetof(f_JT1Effect, srceffect2));
        const int32_t oscillator = i32(base + offsetof(f_JT1Effect, osceffect));
        const int32_t type = i32(base + offsetof(f_JT1Effect, effecttype));
        if(destination < 0 || destination >= J3457_WAVES_INST || source1 < 0 ||
           source1 >= J3457_WAVES_INST || source2 < 0 || source2 >= J3457_WAVES_INST ||
           oscillator < 0 || oscillator > J3457_WAVES_INST || type < 0 ||
           type >= SE_NROFEFFECTS)
            throw std::runtime_error("JXS instrument effect has an invalid wave index");
    }
    return {static_cast<uint8_t>(arpeggio)};
}

void validateNotes(const std::vector<PatternNote>& notes,
                   const std::vector<InstrumentInfo>& instruments,
                   const uint8_t* arpeggios)
{
    for(const PatternNote& row : notes)
    {
        if(row.note == 0) continue;
        if(row.note >= 128 || row.instrument > instruments.size() ||
           (row.script == 1 && row.destination >= 128) || row.script > 76)
            throw std::runtime_error("JXS pattern has an invalid note or instrument");
        const size_t first = row.instrument == 0 ? 0 : row.instrument - 1;
        const size_t last = row.instrument == 0 ? instruments.size() : row.instrument;
        for(size_t instrument = first; instrument < last; ++instrument)
        {
            const size_t arpeggio = instruments[instrument].arpeggio * J3457_STEPS_ARP;
            for(size_t step = 0; step < J3457_STEPS_ARP; ++step)
            {
                const int32_t offset = static_cast<int8_t>(arpeggios[arpeggio + step]);
                const int32_t note = static_cast<int32_t>(row.note) + offset;
                if(note < 0 || note >= 128)
                    throw std::runtime_error("JXS arpeggio moves a note outside the renderer table");
            }
        }
    }
}

void validateJxs(const Bytes& bytes)
{
    Cursor cursor {bytes};
    const uint8_t* header = cursor.take(sizeof(f_JT1Header), "header");
    const int32_t version = readI16(header + offsetof(f_JT1Header, mugiversion));
    const int32_t patterns = readI32(header + offsetof(f_JT1Header, nrofpats));
    const int32_t subsongs = readI32(header + offsetof(f_JT1Header, nrofsongs));
    const int32_t instrumentCount = readI32(header + offsetof(f_JT1Header, nrofinst));
    if((version != 3456 && version != 3457) || patterns <= 0 || patterns > MAX_PATTERNS ||
       subsongs <= 0 || subsongs > MAX_SUBSONGS || instrumentCount <= 0 ||
       instrumentCount > MAX_INSTRUMENTS)
        throw std::runtime_error("JXS header has unsupported version or unsafe object counts");

    for(int32_t index = 0; index < subsongs; ++index)
        validateSubsong(cursor.take(sizeof(f_JT1Subsong), "subsong"), patterns);

    std::vector<PatternNote> notes;
    notes.reserve(static_cast<size_t>(patterns) * J3457_ROWS_PAT);
    for(int32_t pattern = 0; pattern < patterns; ++pattern)
    {
        const uint8_t* rows = cursor.take(sizeof(f_JT1Row) * J3457_ROWS_PAT, "pattern");
        for(size_t row = 0; row < J3457_ROWS_PAT; ++row)
        {
            const uint8_t* data = rows + row * sizeof(f_JT1Row);
            notes.push_back({data[offsetof(f_JT1Row, srcnote)],
                             data[offsetof(f_JT1Row, inst)],
                             data[offsetof(f_JT1Row, dstnote)],
                             data[offsetof(f_JT1Row, script)]});
        }
    }

    for(int32_t pattern = 0; pattern < patterns; ++pattern)
    {
        const int32_t length = readI32(cursor.take(sizeof(int32_t), "pattern-name length"));
        if(length < 0 || length > MAX_NAME_BYTES)
            throw std::runtime_error("JXS pattern name exceeds Kog's limit");
        cursor.take(static_cast<size_t>(length), "pattern name");
    }

    std::vector<InstrumentInfo> instruments;
    instruments.reserve(static_cast<size_t>(instrumentCount));
    for(int32_t index = 0; index < instrumentCount; ++index)
    {
        const uint8_t* instrument = cursor.take(sizeof(f_JT1Inst), "instrument");
        instruments.push_back(validateInstrument(instrument, instrumentCount));
        cursor.take(J3457_WAVES_INST * J3457_SAMPS_WAVE * sizeof(int16_t),
                    "instrument waves");
        if(readI32(instrument + offsetof(f_JT1Inst, hasSampData)) != 0)
        {
            const int32_t length = readI32(instrument + offsetof(f_JT1Inst, samplelength));
            const int32_t start = readI32(instrument + offsetof(f_JT1Inst, startpoint));
            const int32_t loop = readI32(instrument + offsetof(f_JT1Inst, looppoint));
            const int32_t end = readI32(instrument + offsetof(f_JT1Inst, endpoint));
            if(length <= 0 || length > MAX_SAMPLE_BYTES || (length & 1) != 0 || start < 0 ||
               loop < start || end <= start || end > length / 2 || loop >= end)
                throw std::runtime_error("JXS instrument sample range is invalid");
            cursor.take(static_cast<size_t>(length), "instrument sample");
        }
    }
    const uint8_t* arpeggios = cursor.take(J3457_ARPS_SONG * J3457_STEPS_ARP, "arpeggios");
    validateNotes(notes, instruments, arpeggios);
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
    if(error) throw std::runtime_error("cannot inspect JXS file: " + error.message());
    if(length == 0 || length > MAX_FILE_BYTES)
        throw std::runtime_error("JXS file is empty or exceeds Kog's 256 MiB limit");
    Bytes bytes(static_cast<size_t>(length));
    std::ifstream stream(path, std::ios::binary);
    if(!stream || !stream.read(reinterpret_cast<char*>(bytes.data()),
                               static_cast<std::streamsize>(bytes.size())))
        throw std::runtime_error("cannot read JXS file");
    return bytes;
}

std::string titleFor(const JT1Subsong* subsong)
{
    size_t length = 0;
    while(length < SE_NAMELEN_SHORT && subsong->name[length] != '\0') ++length;
    while(length > 0 && static_cast<unsigned char>(subsong->name[length - 1]) <= 0x20U) --length;
    size_t start = 0;
    while(start < length && static_cast<unsigned char>(subsong->name[start]) <= 0x20U) ++start;
    return std::string(subsong->name + start, length - start);
}

struct SongDeleter
{
    void operator()(JT1Song* song) const { jxsfile_freeSong(song); }
};

struct PlayerDeleter
{
    void operator()(JT1Player* player) const { jaytrax_free(player); }
};

int run(int argc, char** argv)
{
    if(argc != 4)
        throw std::runtime_error("usage: kog-syntrax-helper <file> <subsong> <start-frame>");
#ifdef _WIN32
    _setmode(_fileno(stdout), _O_BINARY);
#endif
    const uint64_t selected64 = parseUnsigned(argv[2], "Syntrax subsong");
    const uint64_t startFrame = parseUnsigned(argv[3], "Syntrax start frame");
    if(selected64 > std::numeric_limits<uint32_t>::max())
        throw std::runtime_error("Syntrax subsong exceeds the native API limit");
    const uint32_t selected = static_cast<uint32_t>(selected64);

    Bytes bytes = readFile(fs::u8path(argv[1]));
    validateJxs(bytes);
    JT1Song* rawSong = nullptr;
    const int loadError = jxsfile_readSongMem(bytes.data(), bytes.size(), &rawSong);
    std::unique_ptr<JT1Song, SongDeleter> song(rawSong);
    if(loadError != 0 || !song) throw std::runtime_error("syntrax-c rejected the JXS file");
    if(selected >= static_cast<uint32_t>(song->nrofsongs))
        throw std::runtime_error("requested Syntrax subsong does not exist");

    std::unique_ptr<JT1Player, PlayerDeleter> player(jaytrax_init());
    if(!player || !jaytrax_loadSong(player.get(), song.get()))
        throw std::runtime_error("syntrax-c could not initialize playback");
    jaytrax_setInterpolation(player.get(), ITP_CUBIC);
    const int32_t length = jaytrax_getLength(player.get(), static_cast<int>(selected),
                                             static_cast<int>(LOOP_COUNT), SAMPLE_RATE);
    if(length <= 0)
        throw std::runtime_error("Syntrax duration is invalid or exceeds 30 minutes");
    const uint64_t mainFrames = static_cast<uint64_t>(length);
    const bool needsFade = player->playFlg != 0;
    const uint64_t totalFrames = mainFrames + (needsFade ? FADE_FRAMES : 0U);
    const uint64_t firstFrame = std::min(startFrame, totalFrames);
    const std::string title = titleFor(song->subsongs[selected]);

    jaytrax_changeSubsong(player.get(), static_cast<int>(selected));
    uint64_t skipped = 0;
    while(skipped < firstFrame)
    {
        const int32_t frames = static_cast<int32_t>(std::min<uint64_t>(4096, firstFrame - skipped));
        jaytrax_renderChunk(player.get(), nullptr, frames, SAMPLE_RATE);
        skipped += static_cast<uint32_t>(frames);
    }

    if(std::fwrite(MAGIC.data(), 1, MAGIC.size(), stdout) != MAGIC.size())
        throw std::runtime_error("writing Syntrax helper header failed");
    writeU32(1U);
    writeU32(SAMPLE_RATE);
    writeU32(CHANNELS);
    writeU64(totalFrames);
    writeU64(mainFrames);
    writeU32(static_cast<uint32_t>(song->nrofsongs));
    writeU32(selected);
    writeU32(static_cast<uint32_t>(title.size()));
    if(!title.empty() && std::fwrite(title.data(), 1, title.size(), stdout) != title.size())
        throw std::runtime_error("writing Syntrax title failed");

    std::array<int16_t, 512U * CHANNELS> pcm {};
    std::array<uint8_t, 512U * CHANNELS * sizeof(int16_t)> pcmBytes {};
    uint64_t rendered = firstFrame;
    while(rendered < totalFrames)
    {
        const int32_t frames = static_cast<int32_t>(
            std::min<uint64_t>(pcm.size() / CHANNELS, totalFrames - rendered));
        jaytrax_renderChunk(player.get(), pcm.data(), frames, SAMPLE_RATE);
        const size_t samples = static_cast<size_t>(frames) * CHANNELS;
        for(size_t sample = 0; sample < samples; ++sample)
        {
            const uint16_t value = static_cast<uint16_t>(pcm[sample]);
            pcmBytes[sample * 2U] = static_cast<uint8_t>(value);
            pcmBytes[sample * 2U + 1U] = static_cast<uint8_t>(value >> 8U);
        }
        const size_t bytes = samples * sizeof(int16_t);
        if(std::fwrite(pcmBytes.data(), 1, bytes, stdout) != bytes)
            return 0;
        rendered += static_cast<uint32_t>(frames);
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
        std::fprintf(stderr, "kog-syntrax-helper: %s\n", error.what());
        return 1;
    }
}
