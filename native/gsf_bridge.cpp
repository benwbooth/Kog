#include "gsf_bridge.h"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <exception>
#include <filesystem>
#include <fstream>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#include <mgba/flags.h>
#include <mgba-util/audio-buffer.h>
#include <mgba-util/vfs.h>
#include <mgba/core/config.h>
#include <mgba/core/core.h>
#include <mgba/core/interface.h>

#include "psflib.h"

namespace {

constexpr uint8_t gsf_version = 0x22;
constexpr uint32_t channels = 2;
constexpr size_t maximum_rom_size = 32u * 1024u * 1024u;
constexpr size_t audio_buffer_frames = 4096;
constexpr size_t render_chunk_frames = 2048;
constexpr size_t maximum_empty_emulator_frames = 600;

thread_local std::string last_error;

struct PsfFile {
    explicit PsfFile(const char *path)
        : stream(std::filesystem::u8path(path), std::ios::binary) {}

    std::ifstream stream;
};

struct LoaderState {
    std::vector<uint8_t> rom;
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

int load_gsf(void *context,
             const uint8_t *executable,
             size_t executable_size,
             const uint8_t *,
             size_t) {
    if (executable_size == 0) {
        return 0;
    }
    if (context == nullptr || executable == nullptr || executable_size < 12) {
        return -1;
    }
    LoaderState *state = static_cast<LoaderState *>(context);
    const size_t offset = little_u32(executable + 4) & 0x01ffffffu;
    const size_t declared_size = little_u32(executable + 8);
    if (declared_size == 0 || declared_size > executable_size - 12 ||
        offset > maximum_rom_size || declared_size > maximum_rom_size - offset) {
        return -1;
    }
    const size_t end = offset + declared_size;
    if (state->rom.size() < end) {
        state->rom.resize(end, 0);
    }
    std::copy_n(executable + 12, declared_size, state->rom.begin() + offset);
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

uint64_t frames_from_milliseconds(uint64_t milliseconds, uint32_t sample_rate) {
    if (sample_rate == 0 ||
        milliseconds > std::numeric_limits<uint64_t>::max() / sample_rate) {
        throw std::overflow_error("GSF duration exceeds Kog's frame limit");
    }
    return (milliseconds * sample_rate + 999) / 1000;
}

} // namespace

struct KogGsf {
    std::vector<uint8_t> rom;
    MetadataState metadata;
    mCore *core = nullptr;
    mAVStream stream{};
    uint32_t sample_rate = 0;
    uint64_t main_frames = 0;
    uint64_t fade_frames = 0;
    uint64_t rendered_frames = 0;
    std::vector<int16_t> native_samples;

    ~KogGsf() {
        destroy_core();
    }

    void destroy_core() {
        if (core != nullptr) {
            mCoreConfigDeinit(&core->config);
            core->deinit(core);
            core = nullptr;
        }
    }

    void initialize_core() {
        destroy_core();
        VFile *file = VFileFromConstMemory(rom.data(), rom.size());
        if (file == nullptr) {
            throw std::runtime_error("mGBA could not allocate the GSF ROM view");
        }
        core = mCoreFindVF(file);
        if (core == nullptr) {
            file->close(file);
            throw std::runtime_error("GSF library chain does not contain a recognized GBA ROM");
        }
        if (!core->init(core)) {
            file->close(file);
            core->deinit(core);
            core = nullptr;
            throw std::runtime_error("mGBA could not initialize the GBA core");
        }

        mCoreInitConfig(core, nullptr);
        core->setAVStream(core, &stream);
        core->setAudioBufferSize(core, audio_buffer_frames);
        mCoreOptions options{};
        options.useBios = false;
        options.skipBios = true;
        options.sampleRate = 32768;
        options.volume = 0x100;
        mCoreConfigLoadDefaults(&core->config, &options);
        if (!core->loadROM(core, file)) {
            // The GBA core adopts the VFile even when mapping it fails.
            destroy_core();
            throw std::runtime_error("mGBA rejected the assembled GSF ROM");
        }
        core->reset(core);
        sample_rate = core->audioSampleRate(core);
        if (sample_rate == 0 || sample_rate > 384000) {
            destroy_core();
            throw std::runtime_error("mGBA reported an invalid GSF sample rate");
        }
        rendered_frames = 0;
    }

    size_t render_native(int16_t *output, size_t frames) {
        size_t rendered = 0;
        size_t empty_frames = 0;
        while (rendered < frames) {
            mAudioBuffer *buffer = core->getAudioBuffer(core);
            size_t available = mAudioBufferAvailable(buffer);
            if (available == 0) {
                if (++empty_frames > maximum_empty_emulator_frames) {
                    throw std::runtime_error("mGBA produced no GSF audio frames");
                }
                core->runFrame(core);
                continue;
            }
            empty_frames = 0;
            const size_t requested = std::min(available, frames - rendered);
            const size_t read = mAudioBufferRead(buffer, output + rendered * channels, requested);
            if (read == 0) {
                throw std::runtime_error("mGBA audio buffer stalled during GSF playback");
            }
            rendered += read;
        }
        return rendered;
    }
};

extern "C" KogGsf *kog_gsf_open(const char *path,
                                  uint32_t default_length_milliseconds,
                                  uint32_t default_fade_milliseconds) {
    last_error.clear();
    if (path == nullptr || *path == '\0' || default_length_milliseconds == 0) {
        set_error("invalid GSF open arguments");
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
                                    gsf_version,
                                    load_gsf,
                                    &loader,
                                    load_metadata,
                                    &metadata,
                                    0,
                                    psf_status,
                                    &status);
        if (result != gsf_version) {
            set_error(status.empty() ? "psflib rejected the GSF file" : status);
            return nullptr;
        }
        if (loader.rom.size() < 0xb3 || loader.rom.size() > maximum_rom_size) {
            set_error("GSF library chain contains no bounded GBA ROM image");
            return nullptr;
        }

        std::unique_ptr<KogGsf> decoder(new KogGsf());
        decoder->rom = std::move(loader.rom);
        decoder->metadata = std::move(metadata);
        decoder->initialize_core();

        uint64_t length_milliseconds = decoder->metadata.length_milliseconds;
        uint64_t fade_milliseconds = decoder->metadata.fade_milliseconds;
        if (length_milliseconds == 0) {
            length_milliseconds = default_length_milliseconds;
            fade_milliseconds = default_fade_milliseconds;
        }
        decoder->main_frames = frames_from_milliseconds(length_milliseconds, decoder->sample_rate);
        decoder->fade_frames = frames_from_milliseconds(fade_milliseconds, decoder->sample_rate);
        if (decoder->main_frames >
            static_cast<uint64_t>(std::numeric_limits<int64_t>::max()) - decoder->fade_frames) {
            set_error("GSF duration and fade exceed Kog's frame limit");
            return nullptr;
        }
        return decoder.release();
    } catch (const std::exception &error) {
        set_error(error.what());
        return nullptr;
    } catch (...) {
        set_error("unknown GSF initialization failure");
        return nullptr;
    }
}

extern "C" void kog_gsf_free(KogGsf *decoder) {
    delete decoder;
}

extern "C" uint32_t kog_gsf_sample_rate(const KogGsf *decoder) {
    return decoder == nullptr ? 0 : decoder->sample_rate;
}

extern "C" uint32_t kog_gsf_channels(const KogGsf *decoder) {
    return decoder == nullptr ? 0 : channels;
}

extern "C" uint64_t kog_gsf_total_frames(const KogGsf *decoder) {
    return decoder == nullptr ? 0 : decoder->main_frames + decoder->fade_frames;
}

extern "C" const char *kog_gsf_title(const KogGsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.title.c_str();
}

extern "C" const char *kog_gsf_artist(const KogGsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.artist.c_str();
}

extern "C" const char *kog_gsf_album(const KogGsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.album.c_str();
}

extern "C" const char *kog_gsf_genre(const KogGsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.genre.c_str();
}

extern "C" const char *kog_gsf_date(const KogGsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.date.c_str();
}

extern "C" int64_t kog_gsf_render(KogGsf *decoder, float *output, size_t frames) {
    last_error.clear();
    if (decoder == nullptr || (output == nullptr && frames != 0)) {
        set_error("invalid GSF render arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t remaining = total_frames - std::min(total_frames, decoder->rendered_frames);
    const size_t requested = static_cast<size_t>(std::min<uint64_t>(remaining, frames));
    if (requested == 0) {
        return 0;
    }
    if (requested > std::numeric_limits<size_t>::max() / channels) {
        set_error("GSF render request exceeds mGBA's buffer limit");
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
        set_error("unknown GSF rendering failure");
        return -1;
    }
}

extern "C" int64_t kog_gsf_seek(KogGsf *decoder, uint64_t frame) {
    last_error.clear();
    if (decoder == nullptr) {
        set_error("invalid GSF seek arguments");
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
        decoder->native_samples.resize(render_chunk_frames * channels);
        while (decoder->rendered_frames < target) {
            const size_t chunk = static_cast<size_t>(
                std::min<uint64_t>(render_chunk_frames, target - decoder->rendered_frames));
            decoder->render_native(decoder->native_samples.data(), chunk);
            decoder->rendered_frames += chunk;
        }
        return static_cast<int64_t>(target);
    } catch (const std::exception &error) {
        set_error(error.what());
        return -1;
    } catch (...) {
        set_error("unknown GSF seek failure");
        return -1;
    }
}

extern "C" const char *kog_gsf_last_error(void) {
    return last_error.c_str();
}
