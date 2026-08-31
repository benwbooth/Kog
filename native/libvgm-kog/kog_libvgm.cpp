#include "kog_libvgm.h"

#include <algorithm>
#include <cstdio>
#include <cstring>
#include <exception>
#include <limits>
#include <string>
#include <vector>

#include "../libvgm/player/droplayer.hpp"
#include "../libvgm/player/gymplayer.hpp"
#include "../libvgm/player/playera.hpp"
#include "../libvgm/player/s98player.hpp"
#include "../libvgm/player/vgmplayer.hpp"
#include "../libvgm/utils/MemoryLoader.h"

namespace {

const uint32_t kChannels = 2;
const uint32_t kOutputBits = 32;
const uint32_t kRenderFrames = 2048;
const int32_t kMasterVolume = 0x10000;

void copy_error(char *output, size_t output_size, const std::string &error) {
    if (output == NULL || output_size == 0)
        return;
    const size_t length = std::min(output_size - 1, error.size());
    std::memcpy(output, error.data(), length);
    output[length] = '\0';
}

std::string format_codec(const PLR_SONG_INFO &info) {
    char format[5] = {
        static_cast<char>((info.format >> 24) & 0xFF),
        static_cast<char>((info.format >> 16) & 0xFF),
        static_cast<char>((info.format >> 8) & 0xFF),
        '\0',
        '\0',
    };
    char output[64];
    std::snprintf(
        output,
        sizeof(output),
        "%s v%X.%02X",
        format,
        info.fileVerMaj,
        info.fileVerMin);
    return output;
}

std::string trim_message(const char *message) {
    if (message == NULL)
        return std::string();
    std::string output(message);
    while (!output.empty() && (output.back() == '\r' || output.back() == '\n'))
        output.pop_back();
    return output;
}

} // namespace

struct kog_libvgm {
    std::vector<uint8_t> file_data;
    std::vector<uint8_t> yrw801_rom;
    std::vector<int32_t> render_buffer;
    DATA_LOADER *loader;
    PlayerA player;
    bool started;
    bool ended;
    uint32_t sample_rate;
    uint64_t total_frames;
    std::string title;
    std::string artist;
    std::string album;
    std::string date;
    std::string codec;
    std::string warning;
    std::string last_error;

    kog_libvgm()
        : loader(NULL),
          started(false),
          ended(false),
          sample_rate(0),
          total_frames(0) {}

    ~kog_libvgm() {
        if (started)
            player.Stop();
        if (player.GetPlayer() != NULL)
            player.UnloadFile();
        if (loader != NULL)
            DataLoader_Deinit(loader);
    }

    void append_warning(const std::string &message) {
        if (message.empty())
            return;
        if (!warning.empty())
            warning += "; ";
        warning += message;
    }

    static uint8_t event_callback(
        PlayerBase *,
        void *user_param,
        uint8_t event_type,
        void *) {
        kog_libvgm *decoder = static_cast<kog_libvgm *>(user_param);
        if (event_type == PLREVT_END)
            decoder->ended = true;
        return 0;
    }

    static DATA_LOADER *file_callback(
        void *user_param,
        PlayerBase *,
        const char *file_name) {
        kog_libvgm *decoder = static_cast<kog_libvgm *>(user_param);
        if (file_name == NULL || std::strcmp(file_name, "yrw801.rom") != 0 ||
            decoder->yrw801_rom.empty())
            return NULL;
        DATA_LOADER *rom = MemoryLoader_Init(
            decoder->yrw801_rom.data(),
            static_cast<uint32_t>(decoder->yrw801_rom.size()));
        if (rom == NULL)
            return NULL;
        if (DataLoader_Load(rom) != 0) {
            DataLoader_Deinit(rom);
            return NULL;
        }
        return rom;
    }

    static void log_callback(
        void *user_param,
        PlayerBase *,
        uint8_t level,
        uint8_t,
        const char *source_tag,
        const char *message) {
        if (level > PLRLOG_WARN)
            return;
        kog_libvgm *decoder = static_cast<kog_libvgm *>(user_param);
        std::string output;
        if (source_tag != NULL && source_tag[0] != '\0') {
            output += source_tag;
            output += ": ";
        }
        output += trim_message(message);
        decoder->append_warning(output);
    }

    bool initialize(
        const uint8_t *data,
        size_t data_size,
        const uint8_t *rom,
        size_t rom_size,
        uint32_t output_sample_rate,
        uint32_t loop_count,
        uint32_t fade_samples,
        uint32_t end_silence_samples) {
        if (data == NULL || data_size == 0) {
            last_error = "libvgm input is empty";
            return false;
        }
        if (data_size > std::numeric_limits<uint32_t>::max() ||
            rom_size > std::numeric_limits<uint32_t>::max()) {
            last_error = "libvgm input exceeds the native loader limit";
            return false;
        }
        if (output_sample_rate == 0) {
            last_error = "libvgm sample rate is zero";
            return false;
        }

        file_data.assign(data, data + data_size);
        if (rom != NULL && rom_size != 0)
            yrw801_rom.assign(rom, rom + rom_size);
        sample_rate = output_sample_rate;
        render_buffer.resize(kRenderFrames * kChannels);

        player.RegisterPlayerEngine(new VGMPlayer);
        player.RegisterPlayerEngine(new S98Player);
        player.RegisterPlayerEngine(new DROPlayer);
        player.RegisterPlayerEngine(new GYMPlayer);
        player.SetEventCallback(event_callback, this);
        player.SetFileReqCallback(file_callback, this);
        player.SetLogCallback(log_callback, this);

        PlayerA::Config config = player.GetConfiguration();
        config.masterVol = kMasterVolume;
        config.loopCount = loop_count;
        config.fadeSmpls = fade_samples;
        config.endSilenceSmpls = end_silence_samples;
        config.pbSpeed = 1.0;
        player.SetConfiguration(config);
        if (player.SetOutputSettings(
                sample_rate,
                kChannels,
                kOutputBits,
                kRenderFrames) != 0) {
            last_error = "libvgm rejected Kog's output settings";
            return false;
        }

        loader = MemoryLoader_Init(file_data.data(), static_cast<uint32_t>(file_data.size()));
        if (loader == NULL) {
            last_error = "allocating the libvgm memory loader failed";
            return false;
        }
        DataLoader_SetPreloadBytes(loader, 0x100);
        if (DataLoader_Load(loader) != 0) {
            last_error = "loading the libvgm input failed";
            return false;
        }
        const uint8_t load_result = player.LoadFile(loader);
        if (load_result != 0) {
            char output[96];
            std::snprintf(
                output,
                sizeof(output),
                "libvgm rejected the input (code 0x%02X)",
                load_result);
            last_error = output;
            return false;
        }

        PlayerBase *engine = player.GetPlayer();
        if (engine == NULL) {
            last_error = "libvgm selected no player engine";
            return false;
        }
        if (engine->GetPlayerType() == FCC_VGM) {
            VGMPlayer *vgm_player = dynamic_cast<VGMPlayer *>(engine);
            if (vgm_player != NULL)
                player.SetLoopCount(vgm_player->GetModifiedLoopCount(loop_count));
        } else {
            player.SetLoopCount(loop_count);
        }

        const double seconds = engine->Tick2Second(engine->GetTotalTicks());
        if (seconds < 0.0) {
            last_error = "libvgm returned an invalid duration";
            return false;
        }
        const long double frames =
            static_cast<long double>(seconds) * static_cast<long double>(sample_rate);
        if (frames > static_cast<long double>(std::numeric_limits<uint64_t>::max())) {
            last_error = "libvgm duration exceeds Kog's limit";
            return false;
        }
        total_frames = static_cast<uint64_t>(frames);

        const char *const *tags = engine->GetTags();
        if (tags != NULL) {
            for (const char *const *tag = tags; *tag != NULL; tag += 2) {
                if (tag[1] == NULL)
                    break;
                if (std::strcmp(tag[0], "TITLE") == 0)
                    title = tag[1];
                else if (std::strcmp(tag[0], "ARTIST") == 0)
                    artist = tag[1];
                else if (std::strcmp(tag[0], "GAME") == 0)
                    album = tag[1];
                else if (std::strcmp(tag[0], "DATE") == 0)
                    date = tag[1];
            }
        }
        PLR_SONG_INFO song_info;
        if (engine->GetSongInfo(song_info) == 0)
            codec = format_codec(song_info);
        else
            codec = engine->GetPlayerName();

        const uint8_t start_result = player.Start();
        if (start_result != 0) {
            char output[96];
            std::snprintf(
                output,
                sizeof(output),
                "starting libvgm playback failed (code 0x%02X)",
                start_result);
            last_error = output;
            return false;
        }
        started = true;
        return true;
    }
};

extern "C" kog_libvgm *kog_libvgm_create(
    const uint8_t *data,
    size_t data_size,
    const uint8_t *yrw801_rom,
    size_t yrw801_rom_size,
    uint32_t sample_rate,
    uint32_t loop_count,
    uint32_t fade_samples,
    uint32_t end_silence_samples,
    char *error,
    size_t error_size) {
    try {
        kog_libvgm *decoder = new kog_libvgm;
        if (!decoder->initialize(
                data,
                data_size,
                yrw801_rom,
                yrw801_rom_size,
                sample_rate,
                loop_count,
                fade_samples,
                end_silence_samples)) {
            copy_error(error, error_size, decoder->last_error);
            delete decoder;
            return NULL;
        }
        return decoder;
    } catch (const std::exception &exception) {
        copy_error(error, error_size, exception.what());
        return NULL;
    } catch (...) {
        copy_error(error, error_size, "unknown libvgm exception");
        return NULL;
    }
}

extern "C" void kog_libvgm_destroy(kog_libvgm *decoder) {
    delete decoder;
}

extern "C" uint64_t kog_libvgm_total_frames(const kog_libvgm *decoder) {
    return decoder == NULL ? 0 : decoder->total_frames;
}

extern "C" const char *kog_libvgm_title(const kog_libvgm *decoder) {
    return decoder == NULL ? "" : decoder->title.c_str();
}

extern "C" const char *kog_libvgm_artist(const kog_libvgm *decoder) {
    return decoder == NULL ? "" : decoder->artist.c_str();
}

extern "C" const char *kog_libvgm_album(const kog_libvgm *decoder) {
    return decoder == NULL ? "" : decoder->album.c_str();
}

extern "C" const char *kog_libvgm_date(const kog_libvgm *decoder) {
    return decoder == NULL ? "" : decoder->date.c_str();
}

extern "C" const char *kog_libvgm_codec(const kog_libvgm *decoder) {
    return decoder == NULL ? "" : decoder->codec.c_str();
}

extern "C" const char *kog_libvgm_warning(const kog_libvgm *decoder) {
    return decoder == NULL ? "" : decoder->warning.c_str();
}

extern "C" const char *kog_libvgm_last_error(const kog_libvgm *decoder) {
    return decoder == NULL ? "libvgm decoder is unavailable" : decoder->last_error.c_str();
}

extern "C" size_t kog_libvgm_render(
    kog_libvgm *decoder,
    float *output,
    size_t frames) {
    if (decoder == NULL || output == NULL || frames == 0 || decoder->ended)
        return 0;
    try {
        decoder->last_error.clear();
        const size_t frames_to_render = std::min(frames, static_cast<size_t>(kRenderFrames));
        const uint32_t bytes_requested =
            static_cast<uint32_t>(frames_to_render * kChannels * sizeof(int32_t));
        const uint32_t bytes_rendered =
            decoder->player.Render(bytes_requested, decoder->render_buffer.data());
        const size_t frames_rendered = bytes_rendered / (kChannels * sizeof(int32_t));
        const size_t samples_rendered = frames_rendered * kChannels;
        for (size_t sample = 0; sample < samples_rendered; ++sample) {
            output[sample] = static_cast<float>(
                decoder->render_buffer[sample] * (1.0 / 2147483648.0));
        }
        return frames_rendered;
    } catch (const std::exception &exception) {
        decoder->last_error = exception.what();
        decoder->ended = true;
        return 0;
    } catch (...) {
        decoder->last_error = "unknown libvgm rendering exception";
        decoder->ended = true;
        return 0;
    }
}

extern "C" int kog_libvgm_seek(kog_libvgm *decoder, uint64_t frame) {
    if (decoder == NULL)
        return 1;
    decoder->last_error.clear();
    if (frame > std::numeric_limits<uint32_t>::max()) {
        decoder->last_error = "libvgm seek position exceeds the native API limit";
        return 1;
    }
    try {
        decoder->ended = false;
        const uint8_t result = decoder->player.Seek(PLAYPOS_SAMPLE, static_cast<uint32_t>(frame));
        if (result != 0) {
            char output[96];
            std::snprintf(
                output,
                sizeof(output),
                "libvgm seek failed (code 0x%02X)",
                result);
            decoder->last_error = output;
            return 1;
        }
        return 0;
    } catch (const std::exception &exception) {
        decoder->last_error = exception.what();
        return 1;
    } catch (...) {
        decoder->last_error = "unknown libvgm seek exception";
        return 1;
    }
}
