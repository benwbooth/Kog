/* Kog's in-process FFmpeg streaming encoder.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include "ffmpeg_encoder_bridge.h"

#include <algorithm>
#include <cerrno>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <string>

extern "C" {
#include <libavcodec/avcodec.h>
#include <libavcodec/version_major.h>
#include <libavformat/avformat.h>
#include <libavutil/audio_fifo.h>
#include <libavutil/opt.h>
#include <libavutil/samplefmt.h>
#include <libswresample/swresample.h>
}

namespace {

std::string ffError(int code)
{
    char text[AV_ERROR_MAX_STRING_SIZE] = {};
    av_strerror(code, text, sizeof(text));
    return text;
}

void check(int code, const char* action)
{
    if(code < 0) throw std::runtime_error(std::string(action) + ": " + ffError(code));
}

struct Encoder
{
    AVFormatContext* format = nullptr;
    AVCodecContext* codec = nullptr;
    AVStream* stream = nullptr;
    AVIOContext* io = nullptr;
    SwrContext* resampler = nullptr;
    AVAudioFifo* fifo = nullptr;
    AVPacket* packet = nullptr;
    KogFfmpegWrite write = nullptr;
    void* opaque = nullptr;
    std::string error;
    int inputChannels = 0;
    int64_t nextPts = 0;
    bool finished = false;

    ~Encoder()
    {
        av_packet_free(&packet);
        av_audio_fifo_free(fifo);
        swr_free(&resampler);
        avcodec_free_context(&codec);
        if(format) format->pb = nullptr;
        avio_context_free(&io);
        avformat_free_context(format);
    }

    static int writePacket(void* opaque, const uint8_t* bytes, int count)
    {
        auto& self = *static_cast<Encoder*>(opaque);
        if(self.write(self.opaque, bytes, count) == count) return count;
        self.error = "writing encoded audio failed";
        return AVERROR(EIO);
    }

    void initialize(int kind, int bitrateKbps, int inputRate, int channels)
    {
        if(inputRate < 8000 || inputRate > 384000 || channels < 1 || channels > 8)
            throw std::runtime_error("invalid PCM stream properties");
        if(kind < 0 || kind > 2 || write == nullptr)
            throw std::runtime_error("unsupported stream encoder");
        inputChannels = channels;
        const char* container = kind == 0 ? "adts" : kind == 1 ? "ogg" : "flac";
        const char* encoderName = kind == 0 ? "aac" : kind == 1 ? "libopus" : "flac";
        check(avformat_alloc_output_context2(&format, nullptr, container, nullptr),
              "creating audio container");
        if(!format) throw std::runtime_error("audio container is unavailable");
        const AVCodec* encoder = avcodec_find_encoder_by_name(encoderName);
        if(!encoder && kind == 1) encoder = avcodec_find_encoder(AV_CODEC_ID_OPUS);
        if(!encoder) throw std::runtime_error(std::string(encoderName) + " encoder is unavailable");
        codec = avcodec_alloc_context3(encoder);
        if(!codec) throw std::runtime_error("allocating audio encoder failed");
        codec->codec_type = AVMEDIA_TYPE_AUDIO;
        codec->sample_rate = kind == 1 ? 48000 : inputRate;
#if LIBAVCODEC_VERSION_MAJOR >= 62
        const void* config = nullptr;
        int count = 0;
        if(avcodec_get_supported_config(nullptr, encoder, AV_CODEC_CONFIG_SAMPLE_RATE, 0,
                                        &config, &count) >= 0 && config && count > 0)
        {
            const auto* rates = static_cast<const int*>(config);
            int nearest = rates[0];
            for(int i = 1; i < count; ++i)
                if(std::abs(rates[i] - codec->sample_rate) <
                   std::abs(nearest - codec->sample_rate)) nearest = rates[i];
            codec->sample_rate = nearest;
        }
#else
        if(encoder->supported_samplerates)
        {
            int nearest = encoder->supported_samplerates[0];
            for(const int* rate = encoder->supported_samplerates + 1; *rate; ++rate)
                if(std::abs(*rate - codec->sample_rate) <
                   std::abs(nearest - codec->sample_rate)) nearest = *rate;
            codec->sample_rate = nearest;
        }
#endif
        codec->sample_fmt = kind == 2 ? AV_SAMPLE_FMT_S16 : AV_SAMPLE_FMT_FLTP;
#if LIBAVCODEC_VERSION_MAJOR >= 62
        config = nullptr;
        count = 0;
        if(avcodec_get_supported_config(nullptr, encoder, AV_CODEC_CONFIG_SAMPLE_FORMAT, 0,
                                        &config, &count) >= 0 && config && count > 0)
        {
            const auto* formats = static_cast<const AVSampleFormat*>(config);
            bool preferred = false;
            for(int i = 0; i < count; ++i)
                preferred |= formats[i] == codec->sample_fmt;
            if(!preferred) codec->sample_fmt = formats[0];
        }
#else
        if(encoder->sample_fmts)
        {
            bool preferred = false;
            for(const AVSampleFormat* format = encoder->sample_fmts;
                *format != AV_SAMPLE_FMT_NONE; ++format)
                preferred |= *format == codec->sample_fmt;
            if(!preferred) codec->sample_fmt = encoder->sample_fmts[0];
        }
#endif
        av_channel_layout_default(&codec->ch_layout, channels);
        codec->time_base = AVRational{1, codec->sample_rate};
        if(kind != 2) codec->bit_rate = bitrateKbps * 1000;
        if(format->oformat->flags & AVFMT_GLOBALHEADER)
            codec->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
        if(kind == 1 && std::strcmp(encoder->name, "opus") == 0)
            codec->strict_std_compliance = FF_COMPLIANCE_EXPERIMENTAL;
        check(avcodec_open2(codec, encoder, nullptr), "opening audio encoder");
        stream = avformat_new_stream(format, nullptr);
        if(!stream) throw std::runtime_error("creating audio stream failed");
        stream->time_base = codec->time_base;
        check(avcodec_parameters_from_context(stream->codecpar, codec),
              "configuring audio stream");

        AVChannelLayout sourceLayout {};
        av_channel_layout_default(&sourceLayout, channels);
        const int swrResult = swr_alloc_set_opts2(
            &resampler, &codec->ch_layout, codec->sample_fmt, codec->sample_rate,
            &sourceLayout, AV_SAMPLE_FMT_FLT, inputRate, 0, nullptr);
        av_channel_layout_uninit(&sourceLayout);
        check(swrResult, "configuring audio resampler");
        check(swr_init(resampler), "starting audio resampler");
        fifo = av_audio_fifo_alloc(codec->sample_fmt, channels, 4096);
        if(!fifo) throw std::runtime_error("allocating audio sample queue failed");
        packet = av_packet_alloc();
        if(!packet) throw std::runtime_error("allocating encoded packet failed");
        auto* buffer = static_cast<unsigned char*>(av_malloc(32768));
        if(!buffer) throw std::runtime_error("allocating encoded output buffer failed");
        io = avio_alloc_context(buffer, 32768, 1, this, nullptr, writePacket, nullptr);
        if(!io)
        {
            av_free(buffer);
            throw std::runtime_error("creating encoded output stream failed");
        }
        format->pb = io;
        format->flags |= AVFMT_FLAG_CUSTOM_IO;
        check(avformat_write_header(format, nullptr), "writing audio header");
        avio_flush(io);
        check(io->error, "flushing audio header");
    }

    void drainPackets()
    {
        for(;;)
        {
            const int status = avcodec_receive_packet(codec, packet);
            if(status == AVERROR(EAGAIN) || status == AVERROR_EOF) return;
            check(status, "receiving encoded audio");
            av_packet_rescale_ts(packet, codec->time_base, stream->time_base);
            packet->stream_index = stream->index;
            const int writeStatus = av_interleaved_write_frame(format, packet);
            av_packet_unref(packet);
            check(writeStatus, "writing encoded packet");
            avio_flush(io);
            check(io->error, "flushing encoded packet");
        }
    }

    void emitFrame(int count, bool final)
    {
        const int frameSize = codec->frame_size > 0 ? codec->frame_size : 4096;
        const int samples = final && codec->frame_size > 0 ? frameSize : count;
        AVFrame* frame = av_frame_alloc();
        if(!frame) throw std::runtime_error("allocating audio frame failed");
        frame->nb_samples = samples;
        frame->format = codec->sample_fmt;
        frame->sample_rate = codec->sample_rate;
        av_channel_layout_copy(&frame->ch_layout, &codec->ch_layout);
        const int bufferStatus = av_frame_get_buffer(frame, 0);
        if(bufferStatus < 0)
        {
            av_frame_free(&frame);
            check(bufferStatus, "allocating audio frame buffer");
        }
        const int read = av_audio_fifo_read(fifo, reinterpret_cast<void**>(frame->data), count);
        if(read != count)
        {
            av_frame_free(&frame);
            throw std::runtime_error("reading queued audio samples failed");
        }
        if(samples > count)
            check(av_samples_set_silence(frame->data, count, samples - count,
                                         inputChannels, codec->sample_fmt),
                  "padding the last audio frame");
        frame->pts = nextPts;
        nextPts += samples;
        const int sendStatus = avcodec_send_frame(codec, frame);
        av_frame_free(&frame);
        check(sendStatus, "sending audio to encoder");
        drainPackets();
    }

    void queueSamples(uint8_t** samples, int count)
    {
        if(count == 0) return;
        check(av_audio_fifo_realloc(fifo, av_audio_fifo_size(fifo) + count),
              "growing audio sample queue");
        if(av_audio_fifo_write(fifo, reinterpret_cast<void**>(samples), count) != count)
            throw std::runtime_error("queueing audio samples failed");
        const int frameSize = codec->frame_size > 0 ? codec->frame_size : 4096;
        while(av_audio_fifo_size(fifo) >= frameSize) emitFrame(frameSize, false);
    }

    void push(const float* input, int frames)
    {
        if(finished || frames < 0 || (frames > 0 && !input))
            throw std::runtime_error("invalid audio encoder input");
        if(frames == 0) return;
        const int maximum = swr_get_out_samples(resampler, frames);
        check(maximum, "sizing resampled audio");
        uint8_t** samples = nullptr;
        int lineSize = 0;
        check(av_samples_alloc_array_and_samples(&samples, &lineSize, inputChannels,
                                                  maximum, codec->sample_fmt, 0),
              "allocating resampled audio");
        const uint8_t* source[] = {reinterpret_cast<const uint8_t*>(input)};
        const int converted = swr_convert(resampler, samples, maximum, source, frames);
        try
        {
            check(converted, "resampling audio");
            queueSamples(samples, converted);
        }
        catch(...)
        {
            av_freep(&samples[0]);
            av_freep(&samples);
            throw;
        }
        av_freep(&samples[0]);
        av_freep(&samples);
    }

    void finish()
    {
        if(finished) return;
        for(;;)
        {
            const int maximum = swr_get_out_samples(resampler, 0);
            if(maximum <= 0) break;
            uint8_t** samples = nullptr;
            int lineSize = 0;
            check(av_samples_alloc_array_and_samples(&samples, &lineSize, inputChannels,
                                                      maximum, codec->sample_fmt, 0),
                  "allocating final audio samples");
            const int converted = swr_convert(resampler, samples, maximum, nullptr, 0);
            try
            {
                check(converted, "flushing audio resampler");
                queueSamples(samples, converted);
            }
            catch(...)
            {
                av_freep(&samples[0]);
                av_freep(&samples);
                throw;
            }
            av_freep(&samples[0]);
            av_freep(&samples);
            if(converted == 0) break;
        }
        const int remaining = av_audio_fifo_size(fifo);
        if(remaining > 0) emitFrame(remaining, true);
        check(avcodec_send_frame(codec, nullptr), "flushing audio encoder");
        drainPackets();
        check(av_write_trailer(format), "finishing audio container");
        avio_flush(io);
        check(io->error, "flushing encoded output");
        finished = true;
    }
};

template<typename Function>
int invoke(Encoder* encoder, Function action)
{
    if(!encoder) return -1;
    try
    {
        action();
        return 0;
    }
    catch(const std::exception& failure)
    {
        encoder->error = failure.what();
        return -1;
    }
}

} // namespace

extern "C" void* kog_ffmpeg_encoder_open(int codec, int bitrate_kbps,
                                           int input_rate, int channels,
                                           KogFfmpegWrite write, void* opaque,
                                           char* error, size_t error_capacity)
{
    try
    {
        auto encoder = std::make_unique<Encoder>();
        encoder->write = write;
        encoder->opaque = opaque;
        encoder->initialize(codec, bitrate_kbps, input_rate, channels);
        return encoder.release();
    }
    catch(const std::exception& failure)
    {
        if(error_capacity > 0)
            std::snprintf(error, error_capacity, "%s", failure.what());
        return nullptr;
    }
}

extern "C" int kog_ffmpeg_encoder_push(void* encoder,
                                        const float* interleaved, int frames)
{
    return invoke(static_cast<Encoder*>(encoder), [&] {
        static_cast<Encoder*>(encoder)->push(interleaved, frames);
    });
}

extern "C" int kog_ffmpeg_encoder_finish(void* encoder)
{
    return invoke(static_cast<Encoder*>(encoder), [&] {
        static_cast<Encoder*>(encoder)->finish();
    });
}

extern "C" const char* kog_ffmpeg_encoder_error(const void* encoder)
{
    return encoder ? static_cast<const Encoder*>(encoder)->error.c_str() : "invalid encoder";
}

extern "C" void kog_ffmpeg_encoder_close(void* encoder)
{
    delete static_cast<Encoder*>(encoder);
}
