// Kog adapter for Highly Theoretical and psflib.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#include "sdsf_bridge.h"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <stdexcept>
#include <string>
#include <vector>

#include "psflib.h"
#include "sega.h"

namespace {

constexpr uint8_t ssf_version = 0x11;
constexpr uint8_t dsf_version = 0x12;
constexpr uint32_t sample_rate = 44100;
constexpr uint32_t channels = 2;
constexpr size_t ssf_ram_size = 512u * 1024u;
constexpr size_t dsf_ram_size = 8u * 1024u * 1024u;
constexpr size_t render_chunk_frames = 2048;

thread_local std::string last_error;
std::once_flag sega_init_once;
int sega_init_result = -1;

struct PsfFile {
    explicit PsfFile(const char *path)
        : stream(std::filesystem::u8path(path), std::ios::binary) {}

    std::ifstream stream;
};

struct LoaderState {
    explicit LoaderState(size_t maximum_size)
        : memory(maximum_size, 0), lowest(maximum_size), highest(0) {}

    std::vector<uint8_t> memory;
    size_t lowest;
    size_t highest;
};

struct MetadataState {
    std::string title;
    std::string artist;
    std::string album;
    std::string genre;
    std::string date;
    uint64_t length_milliseconds = 0;
    uint64_t fade_milliseconds = 0;
};

void set_error(const std::string &message) {
    last_error = message;
}

uint32_t little_u32(const uint8_t *data) {
    return static_cast<uint32_t>(data[0]) |
           (static_cast<uint32_t>(data[1]) << 8) |
           (static_cast<uint32_t>(data[2]) << 16) |
           (static_cast<uint32_t>(data[3]) << 24);
}

std::string lowercase(const char *value) {
    std::string result = value == nullptr ? std::string() : std::string(value);
    std::transform(result.begin(), result.end(), result.begin(), [](unsigned char character) {
        return static_cast<char>(std::tolower(character));
    });
    return result;
}

uint64_t parse_time_milliseconds(const char *value) {
    if (value == nullptr) {
        return 0;
    }
    std::string text(value);
    if (const size_t newline = text.find_first_of("\r\n"); newline != std::string::npos) {
        text.resize(newline);
    }
    if (text.empty()) {
        return 0;
    }

    long double total_seconds = 0.0;
    long double multiplier = 1.0;
    size_t end = text.size();
    while (true) {
        const size_t separator = text.rfind(':', end == 0 ? 0 : end - 1);
        const size_t begin = separator == std::string::npos ? 0 : separator + 1;
        const std::string component = text.substr(begin, end - begin);
        try {
            size_t consumed = 0;
            const long double parsed = std::stold(component, &consumed);
            if (consumed != component.size() || parsed < 0.0 || !std::isfinite(parsed)) {
                return 0;
            }
            total_seconds += parsed * multiplier;
        } catch (...) {
            return 0;
        }
        if (separator == std::string::npos) {
            break;
        }
        end = separator;
        multiplier *= 60.0;
    }

    const long double milliseconds = total_seconds * 1000.0;
    if (milliseconds <= 0.0 ||
        milliseconds > static_cast<long double>(std::numeric_limits<uint64_t>::max())) {
        return 0;
    }
    return static_cast<uint64_t>(milliseconds);
}

void *psf_open(void *, const char *path) {
    try {
        std::unique_ptr<PsfFile> file(new PsfFile(path));
        if (!file->stream) {
            return nullptr;
        }
        return file.release();
    } catch (...) {
        return nullptr;
    }
}

size_t psf_read(void *buffer, size_t size, size_t count, void *handle) {
    if (handle == nullptr || buffer == nullptr || size == 0 || count == 0 ||
        count > std::numeric_limits<size_t>::max() / size) {
        return 0;
    }
    PsfFile *file = static_cast<PsfFile *>(handle);
    const size_t bytes = size * count;
    if (bytes > static_cast<size_t>(std::numeric_limits<std::streamsize>::max())) {
        return 0;
    }
    file->stream.read(static_cast<char *>(buffer), static_cast<std::streamsize>(bytes));
    return static_cast<size_t>(file->stream.gcount()) / size;
}

int psf_seek(void *handle, int64_t offset, int origin) {
    if (handle == nullptr) {
        return -1;
    }
    std::ios_base::seekdir direction;
    switch (origin) {
    case SEEK_SET:
        direction = std::ios::beg;
        break;
    case SEEK_CUR:
        direction = std::ios::cur;
        break;
    case SEEK_END:
        direction = std::ios::end;
        break;
    default:
        return -1;
    }
    PsfFile *file = static_cast<PsfFile *>(handle);
    file->stream.clear();
    file->stream.seekg(static_cast<std::streamoff>(offset), direction);
    return file->stream ? 0 : -1;
}

int psf_close(void *handle) {
    delete static_cast<PsfFile *>(handle);
    return 0;
}

long psf_tell(void *handle) {
    if (handle == nullptr) {
        return -1;
    }
    const std::streampos position = static_cast<PsfFile *>(handle)->stream.tellg();
    if (position < 0 || position > std::numeric_limits<long>::max()) {
        return -1;
    }
    return static_cast<long>(position);
}

void psf_status(void *context, const char *message) {
    if (context != nullptr && message != nullptr) {
        static_cast<std::string *>(context)->append(message);
    }
}

int load_sdsf(void *context,
              const uint8_t *executable,
              size_t executable_size,
              const uint8_t *,
              size_t) {
    if (context == nullptr || (executable == nullptr && executable_size != 0)) {
        return -1;
    }
    if (executable_size == 0) {
        return 0;
    }
    if (executable_size < 4) {
        return -1;
    }

    LoaderState *state = static_cast<LoaderState *>(context);
    const size_t start = little_u32(executable) & 0x7fffff;
    const size_t length = executable_size - 4;
    if (start >= state->memory.size() || length > state->memory.size() - start) {
        return -1;
    }
    std::copy_n(executable + 4, length, state->memory.begin() + start);
    state->lowest = std::min(state->lowest, start);
    state->highest = std::max(state->highest, start + length);
    return 0;
}

int load_metadata(void *context, const char *name, const char *value) {
    MetadataState *metadata = static_cast<MetadataState *>(context);
    const std::string key = lowercase(name);
    const std::string text = value == nullptr ? std::string() : std::string(value);
    if (key == "title") {
        metadata->title = text;
    } else if (key == "artist") {
        metadata->artist = text;
    } else if (key == "game" || key == "album") {
        metadata->album = text;
    } else if (key == "genre") {
        metadata->genre = text;
    } else if (key == "year" || key == "date") {
        metadata->date = text;
    } else if (key == "length") {
        metadata->length_milliseconds = parse_time_milliseconds(value);
    } else if (key == "fade") {
        metadata->fade_milliseconds = parse_time_milliseconds(value);
    }
    return 0;
}

uint64_t frames_from_milliseconds(uint64_t milliseconds) {
    if (milliseconds > std::numeric_limits<uint64_t>::max() / sample_rate) {
        throw std::overflow_error("SSF/DSF duration exceeds Kog's frame limit");
    }
    return (milliseconds * sample_rate + 999) / 1000;
}

} // namespace

struct KogSdsf {
    explicit KogSdsf(uint8_t file_version)
        : version(file_version), loader(file_version == ssf_version ? ssf_ram_size : dsf_ram_size) {}

    uint8_t version;
    LoaderState loader;
    MetadataState metadata;
    void *core = nullptr;
    uint64_t main_frames = 0;
    uint64_t fade_frames = 0;
    uint64_t rendered_frames = 0;
    std::vector<int16_t> native_samples;

    ~KogSdsf() {
        destroy_core();
    }

    void destroy_core() {
        std::free(core);
        core = nullptr;
    }

    void initialize_core() {
        destroy_core();
        std::call_once(sega_init_once, [] { sega_init_result = sega_init(); });
        if (sega_init_result != 0) {
            throw std::runtime_error("Highly Theoretical global initialization failed");
        }

        const uint8_t system_version = static_cast<uint8_t>(version - 0x10);
        core = std::calloc(1, sega_get_state_size(system_version));
        if (core == nullptr) {
            throw std::bad_alloc();
        }
        sega_clear_state(core, system_version);
        sega_enable_dry(core, 1);
        sega_enable_dsp(core, 1);
        sega_enable_dsp_dynarec(core, 0);

        const size_t payload_size = loader.highest - loader.lowest;
        if (payload_size > std::numeric_limits<uint32_t>::max() - 4) {
            throw std::overflow_error("SSF/DSF merged program exceeds the emulator API limit");
        }
        std::vector<uint8_t> program(4 + payload_size);
        const uint32_t start = static_cast<uint32_t>(loader.lowest);
        program[0] = static_cast<uint8_t>(start);
        program[1] = static_cast<uint8_t>(start >> 8);
        program[2] = static_cast<uint8_t>(start >> 16);
        program[3] = static_cast<uint8_t>(start >> 24);
        std::copy(loader.memory.begin() + static_cast<std::ptrdiff_t>(loader.lowest),
                  loader.memory.begin() + static_cast<std::ptrdiff_t>(loader.highest),
                  program.begin() + 4);
        if (sega_upload_program(core, program.data(), static_cast<uint32_t>(program.size())) != 0) {
            throw std::runtime_error("Highly Theoretical rejected the merged SSF/DSF program");
        }
        rendered_frames = 0;
    }

    void render_native(int16_t *output, size_t frames) {
        size_t rendered = 0;
        while (rendered < frames) {
            uint32_t chunk = static_cast<uint32_t>(
                std::min<size_t>(frames - rendered, render_chunk_frames));
            int16_t *chunk_output = output == nullptr ? nullptr : output + rendered * channels;
            if (sega_execute(core,
                             std::numeric_limits<int32_t>::max(),
                             chunk_output,
                             &chunk) < 0 ||
                chunk == 0) {
                throw std::runtime_error("Highly Theoretical stalled during SSF/DSF playback");
            }
            rendered += chunk;
        }
    }
};

extern "C" KogSdsf *kog_sdsf_open(const char *path,
                                     uint32_t default_length_milliseconds,
                                     uint32_t default_fade_milliseconds) {
    last_error.clear();
    if (path == nullptr || *path == '\0' || default_length_milliseconds == 0) {
        set_error("invalid SSF/DSF open arguments");
        return nullptr;
    }

    try {
        const psf_file_callbacks callbacks = {
            "/\\", nullptr, psf_open, psf_read, psf_seek, psf_close, psf_tell};
        std::string status;
        const int detected = psf_load(path,
                                      &callbacks,
                                      0,
                                      nullptr,
                                      nullptr,
                                      nullptr,
                                      nullptr,
                                      0,
                                      psf_status,
                                      &status);
        if (detected != ssf_version && detected != dsf_version) {
            set_error(status.empty() ? "psflib did not detect an SSF or DSF file" : status);
            return nullptr;
        }

        std::unique_ptr<KogSdsf> decoder(new KogSdsf(static_cast<uint8_t>(detected)));
        status.clear();
        const int result = psf_load(path,
                                    &callbacks,
                                    detected,
                                    load_sdsf,
                                    &decoder->loader,
                                    load_metadata,
                                    &decoder->metadata,
                                    0,
                                    psf_status,
                                    &status);
        if (result != detected) {
            set_error(status.empty() ? "psflib rejected the SSF/DSF file" : status);
            return nullptr;
        }
        if (decoder->loader.lowest >= decoder->loader.highest) {
            set_error("SSF/DSF library chain contains no executable program");
            return nullptr;
        }
        decoder->initialize_core();

        uint64_t length_milliseconds = decoder->metadata.length_milliseconds;
        uint64_t fade_milliseconds = decoder->metadata.fade_milliseconds;
        if (length_milliseconds == 0) {
            length_milliseconds = default_length_milliseconds;
            fade_milliseconds = default_fade_milliseconds;
        }
        decoder->main_frames = frames_from_milliseconds(length_milliseconds);
        decoder->fade_frames = frames_from_milliseconds(fade_milliseconds);
        if (decoder->main_frames >
            static_cast<uint64_t>(std::numeric_limits<int64_t>::max()) -
                decoder->fade_frames) {
            set_error("SSF/DSF duration and fade exceed Kog's frame limit");
            return nullptr;
        }
        return decoder.release();
    } catch (const std::exception &error) {
        set_error(error.what());
        return nullptr;
    } catch (...) {
        set_error("unknown SSF/DSF initialization failure");
        return nullptr;
    }
}

extern "C" void kog_sdsf_free(KogSdsf *decoder) {
    delete decoder;
}

extern "C" uint32_t kog_sdsf_sample_rate(const KogSdsf *decoder) {
    return decoder == nullptr ? 0 : sample_rate;
}

extern "C" uint32_t kog_sdsf_channels(const KogSdsf *decoder) {
    return decoder == nullptr ? 0 : channels;
}

extern "C" uint64_t kog_sdsf_total_frames(const KogSdsf *decoder) {
    return decoder == nullptr ? 0 : decoder->main_frames + decoder->fade_frames;
}

extern "C" uint8_t kog_sdsf_version(const KogSdsf *decoder) {
    return decoder == nullptr ? 0 : decoder->version;
}

extern "C" const char *kog_sdsf_title(const KogSdsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.title.c_str();
}

extern "C" const char *kog_sdsf_artist(const KogSdsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.artist.c_str();
}

extern "C" const char *kog_sdsf_album(const KogSdsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.album.c_str();
}

extern "C" const char *kog_sdsf_genre(const KogSdsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.genre.c_str();
}

extern "C" const char *kog_sdsf_date(const KogSdsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.date.c_str();
}

extern "C" int64_t kog_sdsf_render(KogSdsf *decoder, float *output, size_t frames) {
    last_error.clear();
    if (decoder == nullptr || (output == nullptr && frames != 0)) {
        set_error("invalid SSF/DSF render arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t remaining = total_frames - std::min(total_frames, decoder->rendered_frames);
    const size_t requested = static_cast<size_t>(std::min<uint64_t>(remaining, frames));
    if (requested == 0) {
        return 0;
    }
    if (requested > std::numeric_limits<size_t>::max() / channels) {
        set_error("SSF/DSF render request exceeds the native buffer limit");
        return -1;
    }

    try {
        decoder->native_samples.resize(requested * channels);
        decoder->render_native(decoder->native_samples.data(), requested);
        for (size_t frame = 0; frame < requested; ++frame) {
            float gain = 1.0f;
            const uint64_t absolute_frame = decoder->rendered_frames + frame;
            if (decoder->fade_frames != 0 && absolute_frame >= decoder->main_frames) {
                gain = static_cast<float>(total_frames - absolute_frame) /
                       static_cast<float>(decoder->fade_frames);
            }
            for (size_t channel = 0; channel < channels; ++channel) {
                const size_t index = frame * channels + channel;
                output[index] = static_cast<float>(decoder->native_samples[index]) *
                                (gain / 32768.0f);
            }
        }
        decoder->rendered_frames += requested;
        return static_cast<int64_t>(requested);
    } catch (const std::exception &error) {
        set_error(error.what());
        return -1;
    } catch (...) {
        set_error("unknown SSF/DSF rendering failure");
        return -1;
    }
}

extern "C" int64_t kog_sdsf_seek(KogSdsf *decoder, uint64_t frame) {
    last_error.clear();
    if (decoder == nullptr) {
        set_error("invalid SSF/DSF seek arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t target = std::min(frame, total_frames);
    if (target == total_frames) {
        decoder->rendered_frames = target;
        return static_cast<int64_t>(target);
    }

    try {
        decoder->initialize_core();
        while (decoder->rendered_frames < target) {
            const size_t chunk = static_cast<size_t>(
                std::min<uint64_t>(render_chunk_frames,
                                   target - decoder->rendered_frames));
            decoder->render_native(nullptr, chunk);
            decoder->rendered_frames += chunk;
        }
        return static_cast<int64_t>(target);
    } catch (const std::exception &error) {
        set_error(error.what());
        return -1;
    } catch (...) {
        set_error("unknown SSF/DSF seek failure");
        return -1;
    }
}

extern "C" const char *kog_sdsf_last_error(void) {
    return last_error.c_str();
}
