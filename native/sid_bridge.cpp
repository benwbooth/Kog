#include "sid_bridge.h"

#include <algorithm>
#include <cstdint>
#include <exception>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <stdexcept>
#include <string>
#include <vector>

#include "residfp.h"
#include "sidplayfp/SidConfig.h"
#include "sidplayfp/SidInfo.h"
#include "sidplayfp/SidTune.h"
#include "sidplayfp/SidTuneInfo.h"
#include "sidplayfp/sidplayfp.h"

namespace {

thread_local std::string last_error;
std::once_flag residfp_warmup_once;

void set_error(const std::string &message) {
    last_error = message;
}

std::string text_or_empty(const char *value) {
    return value == nullptr ? std::string() : std::string(value);
}

void warm_up_residfp() {
    std::call_once(residfp_warmup_once, [] {
        ReSIDfpBuilder builder("Kog reSIDfp warmup");
        builder.create(1);
        if (!builder.getStatus()) {
            throw std::runtime_error(builder.error());
        }
        builder.filter(true);
        builder.filter6581Curve(0.5);
        builder.filter8580Curve(0.5);
    });
}

bool requires_c64_roms(const SidTuneInfo &info) {
    return info.compatibility() == SidTuneInfo::COMPATIBILITY_R64 ||
           info.compatibility() == SidTuneInfo::COMPATIBILITY_BASIC;
}

} // namespace

struct KogSid {
    std::unique_ptr<SidTune> tune;
    std::unique_ptr<ReSIDfpBuilder> builder;
    std::unique_ptr<sidplayfp> engine;
    std::string title;
    std::string artist;
    std::string released;
    std::string format;
    uint32_t sample_rate = 0;
    uint32_t channels = 0;
    uint32_t subsong_count = 0;
    uint32_t selected_subsong = 0;
    uint64_t main_frames = 0;
    uint64_t fade_frames = 0;
    uint64_t rendered_frames = 0;
    std::vector<short> samples;
};

extern "C" KogSid *kog_sid_open(const uint8_t *data,
                                  size_t data_size,
                                  uint32_t subsong,
                                  uint32_t sample_rate,
                                  uint32_t play_seconds,
                                  uint32_t fade_milliseconds) {
    last_error.clear();
    if (data == nullptr || data_size == 0) {
        set_error("SID input is empty");
        return nullptr;
    }
    if (data_size > std::numeric_limits<uint_least32_t>::max()) {
        set_error("SID input exceeds libsidplayfp's size limit");
        return nullptr;
    }
    if (sample_rate < 8000 || sample_rate > 192000 || play_seconds == 0) {
        set_error("SID playback options are outside Cog's supported range");
        return nullptr;
    }

    try {
        warm_up_residfp();

        std::unique_ptr<KogSid> decoder(new KogSid());
        decoder->tune = std::make_unique<SidTune>(
            data, static_cast<uint_least32_t>(data_size));
        if (!decoder->tune->getStatus()) {
            set_error(decoder->tune->statusString());
            return nullptr;
        }

        const SidTuneInfo *info = decoder->tune->getInfo();
        if (info == nullptr || info->songs() == 0) {
            set_error("libsidplayfp found no playable SID subsongs");
            return nullptr;
        }
        if (subsong >= info->songs()) {
            set_error("requested SID subsong is out of range");
            return nullptr;
        }
        if (requires_c64_roms(*info)) {
            set_error("this SID tune requires original C64 ROM images; Kog does not bundle them and user ROM selection is not configured yet");
            return nullptr;
        }

        if (decoder->tune->selectSong(subsong + 1) != subsong + 1) {
            set_error("libsidplayfp could not select the requested SID subsong");
            return nullptr;
        }
        info = decoder->tune->getInfo();
        decoder->subsong_count = info->songs();
        decoder->selected_subsong = subsong;
        decoder->channels = info->sidChips() > 1 ? 2u : 1u;
        decoder->sample_rate = sample_rate;
        decoder->main_frames = static_cast<uint64_t>(sample_rate) * play_seconds;
        decoder->fade_frames =
            (static_cast<uint64_t>(sample_rate) * fade_milliseconds + 999) / 1000;
        decoder->format = text_or_empty(info->formatString());
        if (info->numberOfInfoStrings() > 0) {
            decoder->title = text_or_empty(info->infoString(0));
        }
        if (info->numberOfInfoStrings() > 1) {
            decoder->artist = text_or_empty(info->infoString(1));
        }
        if (info->numberOfInfoStrings() > 2) {
            decoder->released = text_or_empty(info->infoString(2));
        }

        decoder->engine = std::make_unique<sidplayfp>();
        if (!decoder->engine->load(decoder->tune.get())) {
            set_error(decoder->engine->error());
            return nullptr;
        }

        decoder->builder = std::make_unique<ReSIDfpBuilder>("Kog reSIDfp");
        decoder->builder->create(decoder->engine->info().maxsids());
        if (!decoder->builder->getStatus()) {
            set_error(decoder->builder->error());
            return nullptr;
        }
        decoder->builder->filter(true);
        decoder->builder->filter6581Curve(0.5);
        decoder->builder->filter8580Curve(0.5);

        SidConfig config = decoder->engine->config();
        config.frequency = sample_rate;
        config.sidEmulation = decoder->builder.get();
        config.playback = decoder->channels == 2 ? SidConfig::STEREO : SidConfig::MONO;
        if (!decoder->engine->config(config)) {
            set_error(decoder->engine->error());
            return nullptr;
        }

        return decoder.release();
    } catch (const std::exception &error) {
        set_error(error.what());
        return nullptr;
    } catch (...) {
        set_error("unknown libsidplayfp failure");
        return nullptr;
    }
}

extern "C" void kog_sid_free(KogSid *decoder) {
    delete decoder;
}

extern "C" uint32_t kog_sid_sample_rate(const KogSid *decoder) {
    return decoder == nullptr ? 0 : decoder->sample_rate;
}

extern "C" uint32_t kog_sid_channels(const KogSid *decoder) {
    return decoder == nullptr ? 0 : decoder->channels;
}

extern "C" uint32_t kog_sid_subsong_count(const KogSid *decoder) {
    return decoder == nullptr ? 0 : decoder->subsong_count;
}

extern "C" uint32_t kog_sid_selected_subsong(const KogSid *decoder) {
    return decoder == nullptr ? 0 : decoder->selected_subsong;
}

extern "C" uint64_t kog_sid_total_frames(const KogSid *decoder) {
    return decoder == nullptr ? 0 : decoder->main_frames + decoder->fade_frames;
}

extern "C" const char *kog_sid_title(const KogSid *decoder) {
    return decoder == nullptr ? "" : decoder->title.c_str();
}

extern "C" const char *kog_sid_artist(const KogSid *decoder) {
    return decoder == nullptr ? "" : decoder->artist.c_str();
}

extern "C" const char *kog_sid_released(const KogSid *decoder) {
    return decoder == nullptr ? "" : decoder->released.c_str();
}

extern "C" const char *kog_sid_format(const KogSid *decoder) {
    return decoder == nullptr ? "" : decoder->format.c_str();
}

extern "C" int64_t kog_sid_render(KogSid *decoder, float *output, size_t frames) {
    last_error.clear();
    if (decoder == nullptr || (output == nullptr && frames != 0)) {
        set_error("invalid SID render arguments");
        return -1;
    }

    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t remaining = total_frames - std::min(total_frames, decoder->rendered_frames);
    const size_t requested = static_cast<size_t>(
        std::min<uint64_t>(remaining, std::min<uint64_t>(
            frames, std::numeric_limits<uint_least32_t>::max() / decoder->channels)));
    if (requested == 0) {
        return 0;
    }

    try {
        decoder->samples.resize(requested * decoder->channels);
        const uint_least32_t produced_samples = decoder->engine->play(
            decoder->samples.data(),
            static_cast<uint_least32_t>(decoder->samples.size()));
        const size_t produced = produced_samples / decoder->channels;
        if (produced == 0) {
            set_error(decoder->engine->error());
            return -1;
        }

        for (size_t frame = 0; frame < produced; ++frame) {
            if (decoder->channels == 2) {
                short *pair = decoder->samples.data() + frame * 2;
                const int mid = (static_cast<int>(pair[0]) + pair[1]) / 2;
                const int side = (static_cast<int>(pair[0]) - pair[1]) / 4;
                pair[0] = static_cast<short>(mid + side);
                pair[1] = static_cast<short>(mid - side);
            }

            float gain = 1.0f;
            const uint64_t absolute_frame = decoder->rendered_frames + frame;
            if (decoder->fade_frames != 0 && absolute_frame >= decoder->main_frames) {
                gain = static_cast<float>(total_frames - absolute_frame) /
                       static_cast<float>(decoder->fade_frames);
            }
            for (uint32_t channel = 0; channel < decoder->channels; ++channel) {
                const size_t index = frame * decoder->channels + channel;
                output[index] = static_cast<float>(decoder->samples[index]) *
                                (gain / 32768.0f);
            }
        }
        decoder->rendered_frames += produced;
        return static_cast<int64_t>(produced);
    } catch (const std::exception &error) {
        set_error(error.what());
        return -1;
    } catch (...) {
        set_error("unknown libsidplayfp render failure");
        return -1;
    }
}

extern "C" int64_t kog_sid_seek(KogSid *decoder, uint64_t frame) {
    last_error.clear();
    if (decoder == nullptr) {
        set_error("invalid SID seek arguments");
        return -1;
    }

    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t target = std::min(frame, total_frames);
    if (target == total_frames) {
        decoder->rendered_frames = target;
        return static_cast<int64_t>(target);
    }

    try {
        if (!decoder->engine->load(decoder->tune.get())) {
            set_error(decoder->engine->error());
            return -1;
        }
        decoder->rendered_frames = 0;
        std::vector<short> discard(2048 * decoder->channels);

        const uint64_t accelerated_target = target / 32;
        if (!decoder->engine->fastForward(3200)) {
            set_error(decoder->engine->error());
            return -1;
        }
        uint64_t accelerated_done = 0;
        while (accelerated_done < accelerated_target) {
            const uint64_t todo = std::min<uint64_t>(
                2048, accelerated_target - accelerated_done);
            const uint_least32_t produced_samples = decoder->engine->play(
                discard.data(), static_cast<uint_least32_t>(todo * decoder->channels));
            const uint64_t produced = produced_samples / decoder->channels;
            if (produced == 0) {
                decoder->engine->fastForward(100);
                set_error(decoder->engine->error());
                return -1;
            }
            accelerated_done += produced;
        }
        decoder->rendered_frames = accelerated_done * 32;

        if (!decoder->engine->fastForward(100)) {
            set_error(decoder->engine->error());
            return -1;
        }
        const uint64_t remainder = target - decoder->rendered_frames;
        if (remainder != 0) {
            const uint_least32_t produced_samples = decoder->engine->play(
                discard.data(),
                static_cast<uint_least32_t>(remainder * decoder->channels));
            decoder->rendered_frames += produced_samples / decoder->channels;
        }
        return static_cast<int64_t>(decoder->rendered_frames);
    } catch (const std::exception &error) {
        decoder->engine->fastForward(100);
        set_error(error.what());
        return -1;
    } catch (...) {
        decoder->engine->fastForward(100);
        set_error("unknown libsidplayfp seek failure");
        return -1;
    }
}

extern "C" const char *kog_sid_last_error(void) {
    return last_error.c_str();
}

extern "C" const char *kog_sid_version(void) {
    return "2.4.0a";
}
