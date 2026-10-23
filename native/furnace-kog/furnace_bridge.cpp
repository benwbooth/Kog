/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Kog's decoder over Furnace's engine: FamiTracker (.ftm/.0cc/.dnm), Furnace
 * (.fur) and DefleMask (.dmf) modules rendered to stereo float PCM. The engine
 * is driven the way Furnace's own audio export drives it, without its config
 * directory, log output or audio devices.
 */
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

// Built against a copy of engine.h that befriends KogFurnaceAccess, so the
// bridge can use the playback state the engine's own export thread uses.
#include "engine/engine.h"
#include "ta-log.h"

namespace {
struct KogFurnace
{
    DivEngine engine;
    unsigned rate = 44100;
    unsigned loops = 2;
    std::vector<float> left, right;
    size_t pending = 0, pendingAt = 0;
    bool started = false;
    bool ended = false;
    std::string text[6];
};

void copyError(char* error, size_t capacity, const std::string& message)
{
    if(error && capacity)
    {
        std::snprintf(error, capacity, "%s", message.c_str());
    }
}
}

// Furnace's front end defines this; the engine only calls it on config I/O,
// which Kog never does.
void reportError(String) {}

struct KogFurnaceAccess {
static void* open(const unsigned char* data, size_t size, const char* name, unsigned rate,
                       unsigned loops, char* error, size_t capacity)
{
    logLevel = -1;
    if(!data || size < 16 || !rate)
    {
        copyError(error, capacity, "not a tracker module");
        return nullptr;
    }
    auto* furnace = new KogFurnace();
    furnace->rate = rate;
    furnace->loops = loops ? loops : 1;
    DivEngine& e = furnace->engine;
    try
    {
        // Skip the config directory (and its log file): defaults suffice.
        e.configLoaded = true;
        e.registerSystems();
        e.systemsRegistered = true;
        e.registerROMExports();
        e.romExportsRegistered = true;
        e.setAudio(DIV_AUDIO_DUMMY);
        auto* copy = new unsigned char[size];
        std::memcpy(copy, data, size);
        if(!e.load(copy, size, name ? name : "")) // load takes ownership of copy
        {
            copyError(error, capacity, "Furnace could not load the module: " + e.getLastError());
            delete furnace;
            return nullptr;
        }
        if(!e.init())
        {
            copyError(error, capacity, "Furnace could not initialize its engine");
            delete furnace;
            return nullptr;
        }
        // Render ourselves, at our rate, exactly as the audio export does.
        e.deinitAudioBackend();
        e.got.rate = rate;
        for(int i = 0; i < e.song.systemLen; i++)
        {
            e.disCont[i].setRates(rate);
            e.disCont[i].setQuality(e.lowQuality, e.dcHiPass);
        }
    }
    catch(const std::exception& failure)
    {
        copyError(error, capacity, failure.what());
        delete furnace;
        return nullptr;
    }
    furnace->text[0] = e.song.name;
    furnace->text[1] = e.song.author;
    furnace->text[2] = e.song.category;
    furnace->text[3] = e.song.systemName;
    return furnace;
}

static unsigned subsongCount(void* handle)
{
    auto* furnace = static_cast<KogFurnace*>(handle);
    return static_cast<unsigned>(furnace->engine.song.subsong.size());
}

static const char* text(void* handle, unsigned which)
{
    auto* furnace = static_cast<KogFurnace*>(handle);
    if(which == 4)
    {
        furnace->text[4] = furnace->engine.curSubSong ? furnace->engine.curSubSong->name : "";
        return furnace->text[4].c_str();
    }
    return which < 4 ? furnace->text[which].c_str() : "";
}

// Start a subsong and report how many frames it plays before the requested
// number of loops completes (or the song ends), simulated tick by tick
// without synthesizing audio. Returns 0 on failure.
static unsigned long long start(void* handle, unsigned subsong)
{
    auto* furnace = static_cast<KogFurnace*>(handle);
    DivEngine& e = furnace->engine;
    if(subsong >= e.song.subsong.size()) return 0;
    e.changeSongP(subsong);

    unsigned long long frames = 0;
    {
        e.playSub(false);
        unsigned loops = 0;
        // Bound pathological modules: an hour at the slowest plausible tick.
        for(unsigned long long ticks = 0; ticks < 60ULL * 60 * 1000 && e.playing; ++ticks)
        {
            if(e.nextTick())
            {
                if(++loops >= furnace->loops) break;
            }
            frames += e.cycles > 0 ? static_cast<unsigned long long>(e.cycles) : 0;
            e.cycles = 0;
        }
    }
    // Restart from the top for real rendering.
    e.playSub(false);
    e.remainingLoops = -1;
    furnace->pending = 0;
    furnace->pendingAt = 0;
    furnace->started = true;
    furnace->ended = false;
    return frames ? frames : 1;
}

// Render up to `frames` interleaved stereo frames; returns how many were
// written (0 once the song has stopped).
static size_t render(void* handle, float* output, size_t frames)
{
    auto* furnace = static_cast<KogFurnace*>(handle);
    DivEngine& e = furnace->engine;
    if(!furnace->started) return 0;
    size_t written = 0;
    while(written < frames)
    {
        if(furnace->pendingAt < furnace->pending)
        {
            const size_t take = std::min(frames - written, furnace->pending - furnace->pendingAt);
            for(size_t i = 0; i < take; i++)
            {
                const float l = furnace->left[furnace->pendingAt + i];
                const float r = furnace->right[furnace->pendingAt + i];
                output[(written + i) * 2] = l < -1.0f ? -1.0f : (l > 1.0f ? 1.0f : l);
                output[(written + i) * 2 + 1] = r < -1.0f ? -1.0f : (r > 1.0f ? 1.0f : r);
            }
            furnace->pendingAt += take;
            written += take;
            continue;
        }
        if(furnace->ended || !e.playing)
        {
            furnace->ended = true;
            break;
        }
        constexpr unsigned BLOCK = 1024;
        furnace->left.resize(BLOCK);
        furnace->right.resize(BLOCK);
        float* buffers[2] = {furnace->left.data(), furnace->right.data()};
        e.totalProcessed = BLOCK;
        e.nextBuf(nullptr, buffers, 0, 2, BLOCK);
        size_t produced = e.totalProcessed;
        if(produced > BLOCK) produced = BLOCK;
        furnace->pending = produced;
        furnace->pendingAt = 0;
        if(!produced) furnace->ended = true;
    }
    return written;
}

static void close(void* handle)
{
    auto* furnace = static_cast<KogFurnace*>(handle);
    if(!furnace) return;
    furnace->engine.quit(false);
    delete furnace;
}
};

extern "C" {
void* kog_furnace_open(const unsigned char* data, size_t size, const char* name, unsigned rate,
                       unsigned loops, char* error, size_t capacity)
{ return KogFurnaceAccess::open(data, size, name, rate, loops, error, capacity); }
unsigned kog_furnace_subsong_count(void* handle) { return KogFurnaceAccess::subsongCount(handle); }
const char* kog_furnace_text(void* handle, unsigned which) { return KogFurnaceAccess::text(handle, which); }
unsigned long long kog_furnace_start(void* handle, unsigned subsong) { return KogFurnaceAccess::start(handle, subsong); }
size_t kog_furnace_render(void* handle, float* output, size_t frames) { return KogFurnaceAccess::render(handle, output, frames); }
void kog_furnace_close(void* handle) { KogFurnaceAccess::close(handle); }
}
