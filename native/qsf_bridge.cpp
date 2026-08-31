// Kog adapter for Highly Quixotic and psflib.
// Copyright (C) 2026 Kog contributors.
// SPDX-License-Identifier: GPL-3.0-or-later

#include "qsf_bridge.h"

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
#include <string_view>
#include <vector>

#include "psflib.h"
#include "qsound.h"

extern "C" void kog_qsoundc_cleanup(void *state);
extern "C" int kog_qsoundc_has_rom(void *state, uint32_t size);

namespace {

constexpr uint8_t qsf_version = 0x41;
constexpr uint32_t sample_rate = 24038;
constexpr uint32_t channels = 2;
constexpr size_t minimum_z80_rom_size = 0x8000;
constexpr size_t maximum_z80_rom_size = 512u * 1024u;
constexpr size_t maximum_sample_rom_size = 512u * 1024u * 1024u;
constexpr size_t render_chunk_frames = 2048;

thread_local std::string last_error;
std::once_flag qsound_init_once;
int qsound_init_result = -1;

struct PsfFile {
    explicit PsfFile(const char *path)
        : stream(std::filesystem::u8path(path), std::ios::binary) {}

    std::ifstream stream;
};

struct LoaderState {
    std::vector<uint8_t> key;
    std::vector<uint8_t> z80_rom;
    std::vector<uint8_t> sample_rom;
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

uint16_t big_u16(const uint8_t *data) {
    return static_cast<uint16_t>((static_cast<uint16_t>(data[0]) << 8) |
                                 static_cast<uint16_t>(data[1]));
}

uint32_t big_u32(const uint8_t *data) {
    return (static_cast<uint32_t>(data[0]) << 24) |
           (static_cast<uint32_t>(data[1]) << 16) |
           (static_cast<uint32_t>(data[2]) << 8) |
           static_cast<uint32_t>(data[3]);
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

int upload_section(LoaderState *state,
                   std::string_view section,
                   uint32_t offset,
                   const uint8_t *data,
                   uint32_t size) {
    std::vector<uint8_t> *destination = nullptr;
    size_t maximum_size = 0;
    if (section == "KEY") {
        destination = &state->key;
        maximum_size = 11;
    } else if (section == "Z80") {
        destination = &state->z80_rom;
        maximum_size = maximum_z80_rom_size;
    } else if (section == "SMP") {
        destination = &state->sample_rom;
        maximum_size = maximum_sample_rom_size;
    } else {
        return -1;
    }

    const size_t start = offset;
    const size_t length = size;
    if (start > maximum_size || length > maximum_size - start) {
        return -1;
    }
    if (destination->size() < start + length) {
        destination->resize(start + length, 0);
    }
    std::copy_n(data, length, destination->begin() + start);
    return 0;
}

int load_qsf(void *context,
             const uint8_t *executable,
             size_t executable_size,
             const uint8_t *,
             size_t) {
    if (context == nullptr || (executable == nullptr && executable_size != 0)) {
        return -1;
    }
    LoaderState *state = static_cast<LoaderState *>(context);
    while (executable_size != 0) {
        if (executable_size < 11) {
            return -1;
        }
        const std::string_view section(reinterpret_cast<const char *>(executable), 3);
        const uint32_t offset = little_u32(executable + 3);
        const uint32_t size = little_u32(executable + 7);
        executable += 11;
        executable_size -= 11;
        if (size > executable_size ||
            upload_section(state, section, offset, executable, size) != 0) {
            return -1;
        }
        executable += size;
        executable_size -= size;
    }
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
        throw std::overflow_error("QSF duration exceeds Kog's frame limit");
    }
    return (milliseconds * sample_rate + 999) / 1000;
}

} // namespace

struct KogQsf {
    LoaderState loader;
    MetadataState metadata;
    void *core = nullptr;
    uint64_t main_frames = 0;
    uint64_t fade_frames = 0;
    uint64_t rendered_frames = 0;
    std::vector<int16_t> native_samples;

    ~KogQsf() {
        destroy_core();
    }

    void destroy_core() {
        if (core != nullptr) {
            kog_qsoundc_cleanup(qsound_get_qmix_state(core));
            std::free(core);
            core = nullptr;
        }
    }

    void initialize_core() {
        destroy_core();
        std::call_once(qsound_init_once, [] { qsound_init_result = qsound_init(); });
        if (qsound_init_result != 0) {
            throw std::runtime_error("Highly Quixotic global initialization failed");
        }
        core = std::calloc(1, qsound_get_state_size());
        if (core == nullptr) {
            throw std::bad_alloc();
        }
        qsound_clear_state(core);
        if (loader.key.size() == 11) {
            qsound_set_kabuki_key(core,
                                  big_u32(loader.key.data()),
                                  big_u32(loader.key.data() + 4),
                                  big_u16(loader.key.data() + 8),
                                  loader.key[10]);
        } else {
            qsound_set_kabuki_key(core, 0, 0, 0, 0);
        }
        qsound_set_z80_rom(core, loader.z80_rom.data(),
                           static_cast<uint32_t>(loader.z80_rom.size()));
        qsound_set_sample_rom(core, loader.sample_rom.data(),
                              static_cast<uint32_t>(loader.sample_rom.size()));
        if (!kog_qsoundc_has_rom(qsound_get_qmix_state(core),
                                 static_cast<uint32_t>(loader.sample_rom.size()))) {
            throw std::bad_alloc();
        }
        rendered_frames = 0;
    }

    void render_native(int16_t *output, size_t frames) {
        size_t rendered = 0;
        while (rendered < frames) {
            uint32_t chunk = static_cast<uint32_t>(
                std::min<size_t>(frames - rendered, render_chunk_frames));
            int16_t *chunk_output = output == nullptr ? nullptr : output + rendered * channels;
            if (qsound_execute(core,
                               std::numeric_limits<int32_t>::max(),
                               chunk_output,
                               &chunk) < 0 ||
                chunk == 0) {
                throw std::runtime_error("Highly Quixotic stalled during QSF playback");
            }
            rendered += chunk;
        }
    }
};

extern "C" KogQsf *kog_qsf_open(const char *path,
                                  uint32_t default_length_milliseconds,
                                  uint32_t default_fade_milliseconds) {
    last_error.clear();
    if (path == nullptr || *path == '\0' || default_length_milliseconds == 0) {
        set_error("invalid QSF open arguments");
        return nullptr;
    }

    try {
        LoaderState loader;
        MetadataState metadata;
        std::string status;
        const psf_file_callbacks callbacks = {
            "/\\", nullptr, psf_open, psf_read, psf_seek, psf_close, psf_tell};
        const int result = psf_load(path,
                                    &callbacks,
                                    qsf_version,
                                    load_qsf,
                                    &loader,
                                    load_metadata,
                                    &metadata,
                                    0,
                                    psf_status,
                                    &status);
        if (result != qsf_version) {
            set_error(status.empty() ? "psflib rejected the QSF file" : status);
            return nullptr;
        }
        if (loader.z80_rom.size() < minimum_z80_rom_size ||
            loader.z80_rom.size() > maximum_z80_rom_size) {
            set_error("QSF library chain contains no bounded 32 KiB-or-larger Z80 ROM");
            return nullptr;
        }
        if (loader.sample_rom.empty() || loader.sample_rom.size() > maximum_sample_rom_size) {
            set_error("QSF library chain contains no bounded sample ROM");
            return nullptr;
        }

        std::unique_ptr<KogQsf> decoder(new KogQsf());
        decoder->loader = std::move(loader);
        decoder->metadata = std::move(metadata);
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
            set_error("QSF duration and fade exceed Kog's frame limit");
            return nullptr;
        }
        return decoder.release();
    } catch (const std::exception &error) {
        set_error(error.what());
        return nullptr;
    } catch (...) {
        set_error("unknown QSF initialization failure");
        return nullptr;
    }
}

extern "C" void kog_qsf_free(KogQsf *decoder) {
    delete decoder;
}

extern "C" uint32_t kog_qsf_sample_rate(const KogQsf *decoder) {
    return decoder == nullptr ? 0 : sample_rate;
}

extern "C" uint32_t kog_qsf_channels(const KogQsf *decoder) {
    return decoder == nullptr ? 0 : channels;
}

extern "C" uint64_t kog_qsf_total_frames(const KogQsf *decoder) {
    return decoder == nullptr ? 0 : decoder->main_frames + decoder->fade_frames;
}

extern "C" const char *kog_qsf_title(const KogQsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.title.c_str();
}

extern "C" const char *kog_qsf_artist(const KogQsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.artist.c_str();
}

extern "C" const char *kog_qsf_album(const KogQsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.album.c_str();
}

extern "C" const char *kog_qsf_genre(const KogQsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.genre.c_str();
}

extern "C" const char *kog_qsf_date(const KogQsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.date.c_str();
}

extern "C" int64_t kog_qsf_render(KogQsf *decoder, float *output, size_t frames) {
    last_error.clear();
    if (decoder == nullptr || (output == nullptr && frames != 0)) {
        set_error("invalid QSF render arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t remaining = total_frames - std::min(total_frames, decoder->rendered_frames);
    const size_t requested = static_cast<size_t>(std::min<uint64_t>(remaining, frames));
    if (requested == 0) {
        return 0;
    }
    if (requested > std::numeric_limits<size_t>::max() / channels) {
        set_error("QSF render request exceeds the native buffer limit");
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
        set_error("unknown QSF rendering failure");
        return -1;
    }
}

extern "C" int64_t kog_qsf_seek(KogQsf *decoder, uint64_t frame) {
    last_error.clear();
    if (decoder == nullptr) {
        set_error("invalid QSF seek arguments");
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
        set_error("unknown QSF seek failure");
        return -1;
    }
}

extern "C" const char *kog_qsf_last_error(void) {
    return last_error.c_str();
}
