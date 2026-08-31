#include "ffmpeg_bridge.h"

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
#include <libavutil/dict.h>
#include <libavutil/error.h>
#include <libavutil/samplefmt.h>
#include <libavutil/version.h>
#include <libswresample/swresample.h>
}

#include <algorithm>
#include <cerrno>
#include <cmath>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <memory>
#include <string>
#include <vector>

namespace {

thread_local std::string last_open_error;

std::string ffmpeg_error(int code) {
    char buffer[AV_ERROR_MAX_STRING_SIZE] = {};
    if (av_strerror(code, buffer, sizeof(buffer)) < 0) {
        return "FFmpeg error " + std::to_string(code);
    }
    return buffer;
}

std::string dictionary_value(AVDictionary *stream, AVDictionary *container, const char *key) {
    const AVDictionaryEntry *entry = av_dict_get(stream, key, nullptr, 0);
    if (entry == nullptr) {
        entry = av_dict_get(container, key, nullptr, 0);
    }
    return entry != nullptr && entry->value != nullptr ? entry->value : "";
}

uint32_t leading_number(const std::string &value) {
    if (value.empty()) {
        return 0;
    }
    char *end = nullptr;
    errno = 0;
    const unsigned long parsed = std::strtoul(value.c_str(), &end, 10);
    if (errno != 0 || end == value.c_str() || parsed > std::numeric_limits<uint32_t>::max()) {
        return 0;
    }
    return static_cast<uint32_t>(parsed);
}

}  // namespace

struct KogFfmpeg {
    AVFormatContext *format = nullptr;
    AVCodecContext *codec_context = nullptr;
    SwrContext *resampler = nullptr;
    AVPacket *packet = nullptr;
    AVFrame *frame = nullptr;
    int stream_index = -1;
    AVChannelLayout output_layout = {};
    AVChannelLayout input_layout = {};
    bool output_layout_ready = false;
    bool input_layout_ready = false;
    AVSampleFormat input_format = AV_SAMPLE_FMT_NONE;
    int input_rate = 0;
    bool demux_eof = false;
    bool sent_flush = false;
    double seek_target = -1.0;
    std::vector<float> pending;
    size_t pending_offset = 0;
    uint32_t sample_rate = 0;
    uint16_t channels = 0;
    uint32_t bitrate = 0;
    uint8_t bits_per_sample = 0;
    uint32_t year = 0;
    uint32_t track = 0;
    double duration = 0.0;
    std::string codec;
    std::string title;
    std::string artist;
    std::string album;
    std::string genre;
    std::string error;

    ~KogFfmpeg() {
        swr_free(&resampler);
        if (input_layout_ready) {
            av_channel_layout_uninit(&input_layout);
        }
        if (output_layout_ready) {
            av_channel_layout_uninit(&output_layout);
        }
        av_frame_free(&frame);
        av_packet_free(&packet);
        avcodec_free_context(&codec_context);
        avformat_close_input(&format);
    }

    bool configure_resampler(const AVFrame *decoded) {
        AVChannelLayout decoded_layout = decoded->ch_layout;
        if (decoded_layout.nb_channels <= 0) {
            av_channel_layout_default(&decoded_layout, channels);
        }
        const auto decoded_format = static_cast<AVSampleFormat>(decoded->format);
        const int decoded_rate = decoded->sample_rate > 0 ? decoded->sample_rate : sample_rate;
        const bool unchanged = resampler != nullptr && input_layout_ready &&
                               av_channel_layout_compare(&input_layout, &decoded_layout) == 0 &&
                               input_format == decoded_format && input_rate == decoded_rate;
        if (unchanged) {
            if (decoded->ch_layout.nb_channels <= 0) {
                av_channel_layout_uninit(&decoded_layout);
            }
            return true;
        }

        swr_free(&resampler);
        if (input_layout_ready) {
            av_channel_layout_uninit(&input_layout);
            input_layout_ready = false;
        }
        if (av_channel_layout_copy(&input_layout, &decoded_layout) < 0) {
            error = "copying FFmpeg input channel layout";
            if (decoded->ch_layout.nb_channels <= 0) {
                av_channel_layout_uninit(&decoded_layout);
            }
            return false;
        }
        input_layout_ready = true;
        input_format = decoded_format;
        input_rate = decoded_rate;

        const int result = swr_alloc_set_opts2(
            &resampler,
            &output_layout,
            AV_SAMPLE_FMT_FLT,
            static_cast<int>(sample_rate),
            &input_layout,
            input_format,
            input_rate,
            0,
            nullptr);
        if (decoded->ch_layout.nb_channels <= 0) {
            av_channel_layout_uninit(&decoded_layout);
        }
        if (result < 0 || resampler == nullptr) {
            error = "configuring FFmpeg resampler: " + ffmpeg_error(result);
            return false;
        }
        const int init_result = swr_init(resampler);
        if (init_result < 0) {
            error = "initializing FFmpeg resampler: " + ffmpeg_error(init_result);
            return false;
        }
        return true;
    }

    int receive_frame() {
        for (;;) {
            const int receive = avcodec_receive_frame(codec_context, frame);
            if (receive == 0) {
                return 1;
            }
            if (receive == AVERROR_EOF) {
                return 0;
            }
            if (receive != AVERROR(EAGAIN)) {
                error = "decoding audio: " + ffmpeg_error(receive);
                return -1;
            }

            if (demux_eof) {
                if (sent_flush) {
                    return 0;
                }
                const int flush = avcodec_send_packet(codec_context, nullptr);
                if (flush < 0 && flush != AVERROR_EOF) {
                    error = "flushing FFmpeg decoder: " + ffmpeg_error(flush);
                    return -1;
                }
                sent_flush = true;
                continue;
            }

            for (;;) {
                const int read = av_read_frame(format, packet);
                if (read < 0) {
                    demux_eof = true;
                    break;
                }
                if (packet->stream_index == stream_index) {
                    break;
                }
                av_packet_unref(packet);
            }

            if (demux_eof) {
                continue;
            }
            const int send = avcodec_send_packet(codec_context, packet);
            av_packet_unref(packet);
            if (send < 0) {
                error = "submitting an FFmpeg audio packet: " + ffmpeg_error(send);
                return -1;
            }
        }
    }

    int convert_frame() {
        if (!configure_resampler(frame)) {
            return -1;
        }
        const int capacity = av_rescale_rnd(
            swr_get_delay(resampler, input_rate) + frame->nb_samples,
            static_cast<int64_t>(sample_rate),
            input_rate,
            AV_ROUND_UP);
        if (capacity <= 0 || static_cast<uint64_t>(capacity) * channels >
                                 std::numeric_limits<size_t>::max() / sizeof(float)) {
            error = "invalid FFmpeg resampler output size";
            return -1;
        }
        pending.resize(static_cast<size_t>(capacity) * channels);
        uint8_t *output = reinterpret_cast<uint8_t *>(pending.data());
        const int converted = swr_convert(
            resampler,
            &output,
            capacity,
            const_cast<const uint8_t **>(frame->extended_data),
            frame->nb_samples);
        if (converted < 0) {
            error = "converting FFmpeg audio to float: " + ffmpeg_error(converted);
            return -1;
        }
        pending.resize(static_cast<size_t>(converted) * channels);
        pending_offset = 0;

        if (seek_target >= 0.0 && frame->best_effort_timestamp != AV_NOPTS_VALUE) {
            const AVStream *stream = format->streams[stream_index];
            const double frame_start = frame->best_effort_timestamp * av_q2d(stream->time_base);
            if (frame_start < seek_target) {
                const double seconds_to_skip = seek_target - frame_start;
                const uint64_t frames_to_skip = static_cast<uint64_t>(
                    std::ceil(seconds_to_skip * static_cast<double>(sample_rate)));
                pending_offset = std::min(
                    pending.size(),
                    static_cast<size_t>(std::min<uint64_t>(
                        frames_to_skip,
                        std::numeric_limits<size_t>::max() / channels)) * channels);
            }
            const double frame_end = frame_start +
                                     static_cast<double>(converted) /
                                         static_cast<double>(sample_rate);
            if (frame_end >= seek_target) {
                seek_target = -1.0;
            }
        }
        return converted;
    }
};

extern "C" KogFfmpeg *kog_ffmpeg_open(const char *path) {
    last_open_error.clear();
    if (path == nullptr || *path == '\0') {
        last_open_error = "FFmpeg path is empty";
        return nullptr;
    }

    auto decoder = std::make_unique<KogFfmpeg>();
    int result = avformat_open_input(&decoder->format, path, nullptr, nullptr);
    if (result < 0) {
        last_open_error = "opening with FFmpeg: " + ffmpeg_error(result);
        return nullptr;
    }
    result = avformat_find_stream_info(decoder->format, nullptr);
    if (result < 0) {
        last_open_error = "reading FFmpeg stream information: " + ffmpeg_error(result);
        return nullptr;
    }

    const AVCodec *codec = nullptr;
    decoder->stream_index = av_find_best_stream(
        decoder->format, AVMEDIA_TYPE_AUDIO, -1, -1, &codec, 0);
    if (decoder->stream_index < 0 || codec == nullptr) {
        last_open_error = "finding an FFmpeg audio stream: " +
                          ffmpeg_error(decoder->stream_index);
        return nullptr;
    }
    AVStream *stream = decoder->format->streams[decoder->stream_index];
    decoder->codec_context = avcodec_alloc_context3(codec);
    if (decoder->codec_context == nullptr) {
        last_open_error = "allocating the FFmpeg decoder";
        return nullptr;
    }
    result = avcodec_parameters_to_context(decoder->codec_context, stream->codecpar);
    if (result < 0) {
        last_open_error = "copying FFmpeg codec parameters: " + ffmpeg_error(result);
        return nullptr;
    }
    result = avcodec_open2(decoder->codec_context, codec, nullptr);
    if (result < 0) {
        last_open_error = "opening FFmpeg codec " + std::string(codec->name) + ": " +
                          ffmpeg_error(result);
        return nullptr;
    }

    const int channel_count = decoder->codec_context->ch_layout.nb_channels;
    const int sample_rate = decoder->codec_context->sample_rate;
    if (channel_count <= 0 || channel_count > std::numeric_limits<uint16_t>::max() ||
        sample_rate <= 0) {
        last_open_error = "FFmpeg reported invalid audio stream properties";
        return nullptr;
    }
    decoder->channels = static_cast<uint16_t>(channel_count);
    decoder->sample_rate = static_cast<uint32_t>(sample_rate);
    if (av_channel_layout_copy(
            &decoder->output_layout, &decoder->codec_context->ch_layout) < 0) {
        last_open_error = "copying FFmpeg output channel layout";
        return nullptr;
    }
    decoder->output_layout_ready = true;
    decoder->packet = av_packet_alloc();
    decoder->frame = av_frame_alloc();
    if (decoder->packet == nullptr || decoder->frame == nullptr) {
        last_open_error = "allocating FFmpeg packet/frame buffers";
        return nullptr;
    }

    const AVCodecDescriptor *descriptor = avcodec_descriptor_get(
        decoder->codec_context->codec_id);
    decoder->codec = descriptor != nullptr && descriptor->long_name != nullptr
                         ? descriptor->long_name
                         : codec->name;
    decoder->bitrate = static_cast<uint32_t>(std::clamp<int64_t>(
        decoder->codec_context->bit_rate > 0 ? decoder->codec_context->bit_rate
                                             : decoder->format->bit_rate,
        0,
        std::numeric_limits<uint32_t>::max()));
    int bits = decoder->codec_context->bits_per_raw_sample;
    if (bits <= 0) {
        bits = av_get_bits_per_sample(decoder->codec_context->codec_id);
    }
    if (bits <= 0) {
        bits = av_get_bytes_per_sample(decoder->codec_context->sample_fmt) * 8;
    }
    decoder->bits_per_sample = static_cast<uint8_t>(std::clamp(bits, 0, 255));

    if (stream->duration != AV_NOPTS_VALUE) {
        decoder->duration = stream->duration * av_q2d(stream->time_base);
    } else if (decoder->format->duration != AV_NOPTS_VALUE) {
        decoder->duration = static_cast<double>(decoder->format->duration) /
                            static_cast<double>(AV_TIME_BASE);
    }
    decoder->duration = std::max(0.0, decoder->duration);
    decoder->title = dictionary_value(stream->metadata, decoder->format->metadata, "title");
    decoder->artist = dictionary_value(stream->metadata, decoder->format->metadata, "artist");
    decoder->album = dictionary_value(stream->metadata, decoder->format->metadata, "album");
    decoder->genre = dictionary_value(stream->metadata, decoder->format->metadata, "genre");
    decoder->year = leading_number(
        dictionary_value(stream->metadata, decoder->format->metadata, "date"));
    decoder->track = leading_number(
        dictionary_value(stream->metadata, decoder->format->metadata, "track"));
    return decoder.release();
}

extern "C" void kog_ffmpeg_close(KogFfmpeg *decoder) {
    delete decoder;
}

extern "C" const char *kog_ffmpeg_error(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->error.c_str() : last_open_error.c_str();
}

extern "C" const char *kog_ffmpeg_version(void) {
    return av_version_info();
}

extern "C" const char *kog_ffmpeg_codec(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->codec.c_str() : "";
}

extern "C" const char *kog_ffmpeg_title(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->title.c_str() : "";
}

extern "C" const char *kog_ffmpeg_artist(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->artist.c_str() : "";
}

extern "C" const char *kog_ffmpeg_album(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->album.c_str() : "";
}

extern "C" const char *kog_ffmpeg_genre(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->genre.c_str() : "";
}

extern "C" uint32_t kog_ffmpeg_sample_rate(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->sample_rate : 0;
}

extern "C" uint16_t kog_ffmpeg_channels(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->channels : 0;
}

extern "C" uint32_t kog_ffmpeg_bitrate(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->bitrate : 0;
}

extern "C" uint8_t kog_ffmpeg_bits_per_sample(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->bits_per_sample : 0;
}

extern "C" uint32_t kog_ffmpeg_year(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->year : 0;
}

extern "C" uint32_t kog_ffmpeg_track(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->track : 0;
}

extern "C" double kog_ffmpeg_duration(const KogFfmpeg *decoder) {
    return decoder != nullptr ? decoder->duration : 0.0;
}

extern "C" int32_t kog_ffmpeg_render(
    KogFfmpeg *decoder, float *output, uint32_t frames) {
    if (decoder == nullptr || (output == nullptr && frames != 0)) {
        return -1;
    }
    decoder->error.clear();
    uint32_t written = 0;
    while (written < frames) {
        if (decoder->pending_offset < decoder->pending.size()) {
            const size_t available_frames =
                (decoder->pending.size() - decoder->pending_offset) / decoder->channels;
            const size_t copy_frames = std::min<size_t>(available_frames, frames - written);
            const size_t copy_samples = copy_frames * decoder->channels;
            std::copy_n(
                decoder->pending.data() + decoder->pending_offset,
                copy_samples,
                output + static_cast<size_t>(written) * decoder->channels);
            decoder->pending_offset += copy_samples;
            written += static_cast<uint32_t>(copy_frames);
            continue;
        }

        decoder->pending.clear();
        decoder->pending_offset = 0;
        const int received = decoder->receive_frame();
        if (received <= 0) {
            return received < 0 && written == 0 ? -1 : static_cast<int32_t>(written);
        }
        if (decoder->convert_frame() < 0) {
            return written == 0 ? -1 : static_cast<int32_t>(written);
        }
    }
    return static_cast<int32_t>(written);
}

extern "C" int32_t kog_ffmpeg_seek(KogFfmpeg *decoder, double seconds) {
    if (decoder == nullptr || !std::isfinite(seconds) || seconds < 0.0) {
        return -1;
    }
    decoder->error.clear();
    if (decoder->duration > 0.0) {
        seconds = std::min(seconds, decoder->duration);
    }
    const AVStream *stream = decoder->format->streams[decoder->stream_index];
    const int64_t timestamp = static_cast<int64_t>(
        seconds / av_q2d(stream->time_base));
    const int result = avformat_seek_file(
        decoder->format,
        decoder->stream_index,
        std::numeric_limits<int64_t>::min(),
        timestamp,
        timestamp,
        0);
    if (result < 0) {
        decoder->error = "seeking FFmpeg stream: " + ffmpeg_error(result);
        return -1;
    }
    avcodec_flush_buffers(decoder->codec_context);
    swr_free(&decoder->resampler);
    if (decoder->input_layout_ready) {
        av_channel_layout_uninit(&decoder->input_layout);
        decoder->input_layout_ready = false;
    }
    decoder->input_format = AV_SAMPLE_FMT_NONE;
    decoder->input_rate = 0;
    decoder->pending.clear();
    decoder->pending_offset = 0;
    decoder->demux_eof = false;
    decoder->sent_flush = false;
    decoder->seek_target = seconds;
    av_packet_unref(decoder->packet);
    av_frame_unref(decoder->frame);
    return 0;
}
