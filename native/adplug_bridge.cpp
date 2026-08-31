#include "adplug_bridge.h"

#include "adplug.h"
#include "nemuopl.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>
#include <new>
#include <string>
#include <vector>

struct KogAdPlug {
    CPlayer *player = nullptr;
    CNemuopl *emulator = nullptr;
    uint32_t sample_rate = 0;
    uint32_t subsong_count = 0;
    uint32_t subsong = 0;
    uint64_t total_frames = 0;
    uint64_t position = 0;
    uint64_t samples_todo = 0;
    std::string type;
    std::string title;
    std::string author;
    std::vector<int16_t> scratch;
};

static void set_error(int *error, int value) {
    if (error) {
        *error = value;
    }
}

static const std::vector<std::string> &extensions() {
    static const std::vector<std::string> values = [] {
        std::vector<std::string> result;
        for (const CPlayerDesc *player : CAdPlug::players) {
            for (unsigned int index = 0;; ++index) {
                const char *extension = player->get_extension(index);
                if (!extension) {
                    break;
                }
                if (extension[0] == '.') {
                    ++extension;
                }
                std::string value(extension);
                if (!value.empty() &&
                        std::find(result.begin(), result.end(), value) == result.end()) {
                    result.push_back(value);
                }
            }
        }
        return result;
    }();
    return values;
}

static bool ascii_equal(const char *left, const std::string &right) {
    if (!left) {
        return false;
    }
    size_t index = 0;
    while (left[index] && index < right.size()) {
        unsigned char a = static_cast<unsigned char>(left[index]);
        unsigned char b = static_cast<unsigned char>(right[index]);
        if (a >= 'A' && a <= 'Z') {
            a = static_cast<unsigned char>(a - 'A' + 'a');
        }
        if (b >= 'A' && b <= 'Z') {
            b = static_cast<unsigned char>(b - 'A' + 'a');
        }
        if (a != b) {
            return false;
        }
        ++index;
    }
    return left[index] == '\0' && index == right.size();
}

static bool begin_tick(KogAdPlug *decoder) {
    if (!decoder->player->update()) {
        return false;
    }
    const double refresh = decoder->player->getrefresh();
    if (!std::isfinite(refresh) || refresh <= 0.0) {
        return false;
    }
    const double samples = std::ceil(static_cast<double>(decoder->sample_rate) / refresh);
    if (samples < 1.0 || samples > static_cast<double>(std::numeric_limits<int>::max())) {
        return false;
    }
    decoder->samples_todo = static_cast<uint64_t>(samples);
    return true;
}

static size_t render_frames(KogAdPlug *decoder, float *output, size_t frames) {
    const uint64_t available = decoder->total_frames - decoder->position;
    frames = static_cast<size_t>(std::min<uint64_t>(frames, available));
    size_t rendered = 0;

    while (rendered < frames) {
        if (decoder->samples_todo == 0 && !begin_tick(decoder)) {
            break;
        }
        const size_t chunk = static_cast<size_t>(std::min<uint64_t>(
            decoder->samples_todo,
            static_cast<uint64_t>(frames - rendered)
        ));
        decoder->scratch.resize(chunk * 2);
        decoder->emulator->update(decoder->scratch.data(), static_cast<int>(chunk));
        if (output) {
            for (size_t sample = 0; sample < chunk * 2; ++sample) {
                output[rendered * 2 + sample] =
                    static_cast<float>(decoder->scratch[sample]) / 32768.0f;
            }
        }
        decoder->samples_todo -= chunk;
        decoder->position += chunk;
        rendered += chunk;
    }
    return rendered;
}

KogAdPlug *kog_adplug_open(
    const char *path,
    uint32_t subsong,
    uint32_t sample_rate,
    int *error
) {
    KogAdPlug *decoder = nullptr;
    set_error(error, KOG_ADPLUG_OK);
    if (!path || !path[0] || sample_rate < 8000 || sample_rate > 192000) {
        set_error(error, KOG_ADPLUG_INVALID_ARGUMENT);
        return nullptr;
    }

    try {
        decoder = new KogAdPlug;
        decoder->sample_rate = sample_rate;
        decoder->emulator = new CNemuopl(static_cast<int>(sample_rate));
        decoder->player = CAdPlug::factory(path, decoder->emulator);
        if (!decoder->player) {
            kog_adplug_free(decoder);
            set_error(error, KOG_ADPLUG_OPEN_FAILED);
            return nullptr;
        }

        decoder->subsong_count = std::max(1u, decoder->player->getsubsongs());
        if (subsong >= decoder->subsong_count) {
            kog_adplug_free(decoder);
            set_error(error, KOG_ADPLUG_INVALID_SUBSONG);
            return nullptr;
        }
        decoder->subsong = subsong;
        decoder->total_frames =
            static_cast<uint64_t>(decoder->player->songlength(static_cast<int>(subsong))) *
            sample_rate / 1000;
        if (decoder->total_frames == 0) {
            kog_adplug_free(decoder);
            set_error(error, KOG_ADPLUG_OPEN_FAILED);
            return nullptr;
        }

        decoder->type = decoder->player->gettype();
        decoder->title = decoder->player->gettitle();
        decoder->author = decoder->player->getauthor();
        decoder->emulator->init();
        decoder->player->rewind(static_cast<int>(subsong));
        return decoder;
    } catch (const std::bad_alloc &) {
        kog_adplug_free(decoder);
        set_error(error, KOG_ADPLUG_OUT_OF_MEMORY);
        return nullptr;
    } catch (...) {
        kog_adplug_free(decoder);
        set_error(error, KOG_ADPLUG_OPEN_FAILED);
        return nullptr;
    }
}

void kog_adplug_free(KogAdPlug *decoder) {
    if (!decoder) {
        return;
    }
    delete decoder->player;
    delete decoder->emulator;
    delete decoder;
}

uint32_t kog_adplug_sample_rate(const KogAdPlug *decoder) {
    return decoder ? decoder->sample_rate : 0;
}

uint32_t kog_adplug_subsong_count(const KogAdPlug *decoder) {
    return decoder ? decoder->subsong_count : 0;
}

uint64_t kog_adplug_total_frames(const KogAdPlug *decoder) {
    return decoder ? decoder->total_frames : 0;
}

const char *kog_adplug_type(const KogAdPlug *decoder) {
    return decoder ? decoder->type.c_str() : nullptr;
}

const char *kog_adplug_title(const KogAdPlug *decoder) {
    return decoder ? decoder->title.c_str() : nullptr;
}

const char *kog_adplug_author(const KogAdPlug *decoder) {
    return decoder ? decoder->author.c_str() : nullptr;
}

int64_t kog_adplug_render(KogAdPlug *decoder, float *output, size_t frames) {
    if (!decoder || !output || frames == 0 || frames > static_cast<size_t>(INT64_MAX)) {
        return -1;
    }
    try {
        return static_cast<int64_t>(render_frames(decoder, output, frames));
    } catch (...) {
        return -1;
    }
}

int64_t kog_adplug_seek(KogAdPlug *decoder, uint64_t frame) {
    if (!decoder) {
        return -1;
    }
    frame = std::min(frame, decoder->total_frames);
    try {
        decoder->emulator->init();
        decoder->player->rewind(static_cast<int>(decoder->subsong));
        decoder->position = 0;
        decoder->samples_todo = 0;
        while (decoder->position < frame) {
            const size_t chunk = static_cast<size_t>(std::min<uint64_t>(
                frame - decoder->position,
                4096
            ));
            if (render_frames(decoder, nullptr, chunk) == 0) {
                break;
            }
        }
        return static_cast<int64_t>(decoder->position);
    } catch (...) {
        return -1;
    }
}

int kog_adplug_supports_extension(const char *extension) {
    try {
        for (const std::string &candidate : extensions()) {
            if (ascii_equal(extension, candidate)) {
                return 1;
            }
        }
    } catch (...) {
        return 0;
    }
    return 0;
}

size_t kog_adplug_extension_count(void) {
    try {
        return extensions().size();
    } catch (...) {
        return 0;
    }
}

const char *kog_adplug_extension(size_t index) {
    try {
        const std::vector<std::string> &values = extensions();
        return index < values.size() ? values[index].c_str() : nullptr;
    } catch (...) {
        return nullptr;
    }
}

const char *kog_adplug_version(void) {
    try {
        static const std::string version = CAdPlug::get_version();
        return version.c_str();
    } catch (...) {
        return nullptr;
    }
}
