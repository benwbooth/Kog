// Kog adapter for LazyUSF2 and psflib.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#include "usf_bridge.h"

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
#include <new>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include "psflib.h"
#include "usf/usf.h"

namespace {

constexpr uint8_t usf_version = 0x21;
constexpr uint32_t sample_rate = 44100;
constexpr uint32_t channels = 2;
constexpr uint32_t sr64_marker = 0x34365253;
constexpr uint64_t maximum_rom_size = 64u * 1024u * 1024u;
constexpr uint64_t maximum_save_state_size = 0x80275c;
constexpr size_t seek_chunk_frames = 1024;

thread_local std::string last_error;

struct PsfFile {
    explicit PsfFile(const char *path)
        : stream(std::filesystem::u8path(path), std::ios::binary) {}

    std::ifstream stream;
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

struct PlaybackOptions {
    bool enable_compare = false;
    bool enable_fifo_full = false;
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

bool read_u32(const uint8_t *data, size_t size, size_t &position, uint32_t &value) {
    if (position > size || size - position < 4) {
        return false;
    }
    value = little_u32(data + position);
    position += 4;
    return true;
}

bool validate_blocks(const uint8_t *data,
                     size_t size,
                     size_t &position,
                     uint64_t maximum_end) {
    uint32_t length = 0;
    if (!read_u32(data, size, position, length)) {
        return false;
    }
    while (length != 0) {
        uint32_t start = 0;
        if (!read_u32(data, size, position, start)) {
            return false;
        }
        const uint64_t end = static_cast<uint64_t>(start) + length;
        if (end > maximum_end || position > size || length > size - position) {
            return false;
        }
        position += length;
        if (!read_u32(data, size, position, length)) {
            return false;
        }
    }
    return true;
}

bool validate_reserved_section(const uint8_t *data, size_t size) {
    if (size == 0) {
        return true;
    }
    if (data == nullptr) {
        return false;
    }

    size_t position = 0;
    uint32_t marker = 0;
    if (!read_u32(data, size, position, marker)) {
        return false;
    }
    bool saw_section = false;
    if (marker == sr64_marker) {
        saw_section = true;
        if (!validate_blocks(data, size, position, maximum_rom_size)) {
            return false;
        }
    }
    if (!read_u32(data, size, position, marker)) {
        return false;
    }
    if (marker == sr64_marker) {
        saw_section = true;
        if (!validate_blocks(data, size, position, maximum_save_state_size)) {
            return false;
        }
    }
    return saw_section;
}

} // namespace

struct KogUsf {
    KogUsf() {
        core = std::malloc(usf_get_state_size());
        if (core == nullptr) {
            throw std::bad_alloc();
        }
        usf_clear(core);
        usf_set_hle_audio(core, 1);
    }

    ~KogUsf() {
        if (core != nullptr) {
            usf_shutdown(core);
            std::free(core);
        }
    }

    void *core = nullptr;
    MetadataState metadata;
    bool uploaded_data = false;
    uint64_t main_frames = 0;
    uint64_t fade_frames = 0;
    uint64_t rendered_frames = 0;
    std::vector<int16_t> native_samples;

    void render_native(int16_t *output, size_t frames) {
        if (const char *error = usf_render_resampled(core, output, frames, sample_rate)) {
            throw std::runtime_error(error);
        }
    }

    void restart() {
        usf_restart(core);
        rendered_frames = 0;
    }
};

namespace {

int load_usf(void *context,
             const uint8_t *executable,
             size_t executable_size,
             const uint8_t *reserved,
             size_t reserved_size) {
    if (context == nullptr || (executable != nullptr && executable_size != 0)) {
        return -1;
    }
    if (reserved_size == 0) {
        return 0;
    }
    if (!validate_reserved_section(reserved, reserved_size)) {
        return -1;
    }
    KogUsf *decoder = static_cast<KogUsf *>(context);
    if (usf_upload_section(decoder->core, reserved, reserved_size) != 0) {
        return -1;
    }
    decoder->uploaded_data = true;
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

int load_options(void *context, const char *name, const char *value) {
    PlaybackOptions *options = static_cast<PlaybackOptions *>(context);
    const std::string key = lowercase(name);
    const bool enabled = value != nullptr && *value != '\0';
    if (key == "_enablecompare" && enabled) {
        options->enable_compare = true;
    } else if (key == "_enablefifofull" && enabled) {
        options->enable_fifo_full = true;
    }
    return 0;
}

uint64_t frames_from_milliseconds(uint64_t milliseconds) {
    if (milliseconds > std::numeric_limits<uint64_t>::max() / sample_rate) {
        throw std::overflow_error("USF duration exceeds Kog's frame limit");
    }
    return (milliseconds * sample_rate + 999) / 1000;
}

} // namespace

extern "C" KogUsf *kog_usf_open(const char *path,
                                  uint32_t default_length_milliseconds,
                                  uint32_t default_fade_milliseconds) {
    last_error.clear();
    if (path == nullptr || *path == '\0' || default_length_milliseconds == 0) {
        set_error("invalid USF open arguments");
        return nullptr;
    }

    try {
        const psf_file_callbacks callbacks = {
            "/\\", nullptr, psf_open, psf_read, psf_seek, psf_close, psf_tell};
        std::string status;
        MetadataState metadata;
        const int detected = psf_load(path,
                                      &callbacks,
                                      0,
                                      nullptr,
                                      nullptr,
                                      load_metadata,
                                      &metadata,
                                      0,
                                      psf_status,
                                      &status);
        if (detected != usf_version) {
            set_error(status.empty() ? "psflib did not detect a USF file" : status);
            return nullptr;
        }

        std::unique_ptr<KogUsf> decoder(new KogUsf());
        decoder->metadata = std::move(metadata);
        PlaybackOptions options;
        status.clear();
        const int result = psf_load(path,
                                    &callbacks,
                                    usf_version,
                                    load_usf,
                                    decoder.get(),
                                    load_options,
                                    &options,
                                    1,
                                    psf_status,
                                    &status);
        if (result != usf_version) {
            set_error(status.empty() ? "psflib rejected the USF file" : status);
            return nullptr;
        }
        if (!decoder->uploaded_data) {
            set_error("USF library chain contains no ROM or save-state data");
            return nullptr;
        }

        usf_set_compare(decoder->core, options.enable_compare ? 1 : 0);
        usf_set_fifo_full(decoder->core, options.enable_fifo_full ? 1 : 0);

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
            set_error("USF duration and fade exceed Kog's frame limit");
            return nullptr;
        }
        return decoder.release();
    } catch (const std::exception &error) {
        set_error(error.what());
        return nullptr;
    } catch (...) {
        set_error("unknown USF initialization failure");
        return nullptr;
    }
}

extern "C" void kog_usf_free(KogUsf *decoder) {
    delete decoder;
}

extern "C" uint32_t kog_usf_sample_rate(const KogUsf *decoder) {
    return decoder == nullptr ? 0 : sample_rate;
}

extern "C" uint32_t kog_usf_channels(const KogUsf *decoder) {
    return decoder == nullptr ? 0 : channels;
}

extern "C" uint64_t kog_usf_total_frames(const KogUsf *decoder) {
    return decoder == nullptr ? 0 : decoder->main_frames + decoder->fade_frames;
}

extern "C" const char *kog_usf_title(const KogUsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.title.c_str();
}

extern "C" const char *kog_usf_artist(const KogUsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.artist.c_str();
}

extern "C" const char *kog_usf_album(const KogUsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.album.c_str();
}

extern "C" const char *kog_usf_genre(const KogUsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.genre.c_str();
}

extern "C" const char *kog_usf_date(const KogUsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.date.c_str();
}

extern "C" int64_t kog_usf_render(KogUsf *decoder, float *output, size_t frames) {
    last_error.clear();
    if (decoder == nullptr || (output == nullptr && frames != 0)) {
        set_error("invalid USF render arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t remaining = total_frames - std::min(total_frames, decoder->rendered_frames);
    const size_t requested = static_cast<size_t>(std::min<uint64_t>(remaining, frames));
    if (requested == 0) {
        return 0;
    }
    if (requested > std::numeric_limits<size_t>::max() / channels) {
        set_error("USF render request exceeds the native buffer limit");
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
        set_error("unknown USF rendering failure");
        return -1;
    }
}

extern "C" int64_t kog_usf_seek(KogUsf *decoder, uint64_t frame) {
    last_error.clear();
    if (decoder == nullptr) {
        set_error("invalid USF seek arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t target = std::min(frame, total_frames);
    if (target == total_frames) {
        decoder->rendered_frames = target;
        return static_cast<int64_t>(target);
    }

    try {
        decoder->restart();
        while (decoder->rendered_frames < target) {
            const size_t chunk = static_cast<size_t>(
                std::min<uint64_t>(seek_chunk_frames, target - decoder->rendered_frames));
            decoder->render_native(nullptr, chunk);
            decoder->rendered_frames += chunk;
        }
        return static_cast<int64_t>(target);
    } catch (const std::exception &error) {
        set_error(error.what());
        return -1;
    } catch (...) {
        set_error("unknown USF seek failure");
        return -1;
    }
}

extern "C" const char *kog_usf_last_error(void) {
    return last_error.c_str();
}
