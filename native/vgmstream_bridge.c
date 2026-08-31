#include "vgmstream_bridge.h"

#include "libvgmstream.h"

#include <ctype.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define KOG_TAG_TEXT_MAX 2048

struct KogVgmstream {
    libvgmstream_t *stream;
    char title[KOG_TAG_TEXT_MAX];
    char artist[KOG_TAG_TEXT_MAX];
    char album[KOG_TAG_TEXT_MAX];
    uint32_t year;
    uint32_t track_number;
    uint32_t selected_subsong;
};

static void set_error(int *error, int value) {
    if (error) {
        *error = value;
    }
}

static int ascii_equal(const char *left, const char *right) {
    unsigned char a;
    unsigned char b;
    if (!left || !right) {
        return 0;
    }
    while (*left && *right) {
        a = (unsigned char)*left++;
        b = (unsigned char)*right++;
        if (tolower(a) != tolower(b)) {
            return 0;
        }
    }
    return *left == '\0' && *right == '\0';
}

static void copy_text(char *output, size_t output_size, const char *value) {
    if (!output || output_size == 0 || !value) {
        return;
    }
    snprintf(output, output_size, "%s", value);
}

static uint32_t parse_first_number(const char *value) {
    unsigned long parsed;
    char *end;
    if (!value) {
        return 0;
    }
    parsed = strtoul(value, &end, 10);
    if (end == value || parsed > UINT32_MAX) {
        return 0;
    }
    return (uint32_t)parsed;
}

static uint32_t parse_year(const char *value) {
    const char *cursor = value;
    if (!value) {
        return 0;
    }
    while (cursor[0] && cursor[1] && cursor[2] && cursor[3]) {
        if (isdigit((unsigned char)cursor[0]) && isdigit((unsigned char)cursor[1]) &&
                isdigit((unsigned char)cursor[2]) && isdigit((unsigned char)cursor[3])) {
            char year[5];
            memcpy(year, cursor, 4);
            year[4] = '\0';
            return parse_first_number(year);
        }
        ++cursor;
    }
    return 0;
}

static char *tag_path_for(const char *path) {
    const char *forward;
    const char *backward;
    const char *separator;
    const char *tag_name = "!tags.m3u";
    size_t prefix_size;
    size_t output_size;
    char *output;

    forward = strrchr(path, '/');
    backward = strrchr(path, '\\');
    separator = forward;
    if (!separator || (backward && backward > separator)) {
        separator = backward;
    }
    prefix_size = separator ? (size_t)(separator - path + 1) : 0;
    output_size = prefix_size + strlen(tag_name) + 1;
    output = malloc(output_size);
    if (!output) {
        return NULL;
    }
    if (prefix_size > 0) {
        memcpy(output, path, prefix_size);
    }
    memcpy(output + prefix_size, tag_name, strlen(tag_name) + 1);
    return output;
}

static const char *base_name(const char *path) {
    const char *forward = strrchr(path, '/');
    const char *backward = strrchr(path, '\\');
    const char *separator = forward;
    if (!separator || (backward && backward > separator)) {
        separator = backward;
    }
    return separator ? separator + 1 : path;
}

static void read_tags(KogVgmstream *decoder, const char *path) {
    char *tag_path = tag_path_for(path);
    libstreamfile_t *tag_file;
    libvgmstream_tags_t *tags;

    if (!tag_path) {
        return;
    }
    tag_file = libstreamfile_open_from_stdio(tag_path);
    free(tag_path);
    if (!tag_file) {
        return;
    }
    tags = libvgmstream_tags_init(tag_file);
    if (!tags) {
        libstreamfile_close(tag_file);
        return;
    }

    libvgmstream_tags_find(tags, base_name(path));
    while (libvgmstream_tags_next_tag(tags)) {
        if (ascii_equal(tags->key, "TITLE")) {
            copy_text(decoder->title, sizeof(decoder->title), tags->val);
        } else if (ascii_equal(tags->key, "ARTIST")) {
            copy_text(decoder->artist, sizeof(decoder->artist), tags->val);
        } else if (ascii_equal(tags->key, "ALBUM")) {
            copy_text(decoder->album, sizeof(decoder->album), tags->val);
        } else if (ascii_equal(tags->key, "DATE")) {
            decoder->year = parse_year(tags->val);
        } else if (ascii_equal(tags->key, "TRACK") ||
                ascii_equal(tags->key, "TRACKNUMBER")) {
            decoder->track_number = parse_first_number(tags->val);
        }
    }
    libvgmstream_tags_free(tags);
    libstreamfile_close(tag_file);
}

KogVgmstream *kog_vgmstream_open(
    const char *path,
    int32_t subsong,
    double loop_count,
    double fade_seconds,
    int *error
) {
    libstreamfile_t *file;
    libvgmstream_config_t config = {0};
    libvgmstream_t *stream;
    KogVgmstream *decoder;
    int native_subsong;

    set_error(error, KOG_VGMSTREAM_OK);
    if (!path || !path[0] || subsong < -1 || loop_count <= 0.0 || fade_seconds < 0.0) {
        set_error(error, KOG_VGMSTREAM_INVALID_ARGUMENT);
        return NULL;
    }
    if (subsong >= INT_MAX) {
        set_error(error, KOG_VGMSTREAM_INVALID_ARGUMENT);
        return NULL;
    }

    file = libstreamfile_open_from_stdio(path);
    if (!file) {
        set_error(error, KOG_VGMSTREAM_OPEN_FAILED);
        return NULL;
    }
    config.loop_count = loop_count;
    config.fade_time = fade_seconds;
    config.fade_delay = 0.0;
    config.auto_downmix_channels = 6;
    config.force_sfmt = LIBVGMSTREAM_SFMT_FLOAT;
    native_subsong = subsong < 0 ? 0 : subsong + 1;
    stream = libvgmstream_create(file, native_subsong, &config);
    libstreamfile_close(file);
    if (!stream || !stream->format || stream->format->sample_format != LIBVGMSTREAM_SFMT_FLOAT ||
            stream->format->sample_rate <= 0 || stream->format->channels <= 0 ||
            stream->format->play_samples <= 0) {
        libvgmstream_free(stream);
        set_error(error, KOG_VGMSTREAM_OPEN_FAILED);
        return NULL;
    }

    decoder = calloc(1, sizeof(*decoder));
    if (!decoder) {
        libvgmstream_free(stream);
        set_error(error, KOG_VGMSTREAM_DECODE_FAILED);
        return NULL;
    }
    decoder->stream = stream;
    decoder->selected_subsong = subsong < 0
        ? (stream->format->subsong_index > 0 ? (uint32_t)stream->format->subsong_index - 1 : 0)
        : (uint32_t)subsong;
    read_tags(decoder, path);
    return decoder;
}

void kog_vgmstream_free(KogVgmstream *decoder) {
    if (!decoder) {
        return;
    }
    libvgmstream_free(decoder->stream);
    free(decoder);
}

uint32_t kog_vgmstream_sample_rate(const KogVgmstream *decoder) {
    return decoder ? (uint32_t)decoder->stream->format->sample_rate : 0;
}

uint32_t kog_vgmstream_channels(const KogVgmstream *decoder) {
    return decoder ? (uint32_t)decoder->stream->format->channels : 0;
}

uint64_t kog_vgmstream_total_frames(const KogVgmstream *decoder) {
    return decoder ? (uint64_t)decoder->stream->format->play_samples : 0;
}

uint32_t kog_vgmstream_subsong_count(const KogVgmstream *decoder) {
    if (!decoder || decoder->stream->format->subsong_count <= 0) {
        return decoder ? 1 : 0;
    }
    return (uint32_t)decoder->stream->format->subsong_count;
}

uint32_t kog_vgmstream_selected_subsong(const KogVgmstream *decoder) {
    return decoder ? decoder->selected_subsong : 0;
}

uint32_t kog_vgmstream_bitrate(const KogVgmstream *decoder) {
    if (!decoder || decoder->stream->format->stream_bitrate <= 0) {
        return 0;
    }
    return (uint32_t)decoder->stream->format->stream_bitrate / 1000;
}

const char *kog_vgmstream_codec(const KogVgmstream *decoder) {
    return decoder ? decoder->stream->format->codec_name : NULL;
}

const char *kog_vgmstream_title(const KogVgmstream *decoder) {
    return decoder ? decoder->title : NULL;
}

const char *kog_vgmstream_artist(const KogVgmstream *decoder) {
    return decoder ? decoder->artist : NULL;
}

const char *kog_vgmstream_album(const KogVgmstream *decoder) {
    return decoder ? decoder->album : NULL;
}

uint32_t kog_vgmstream_year(const KogVgmstream *decoder) {
    return decoder ? decoder->year : 0;
}

uint32_t kog_vgmstream_track_number(const KogVgmstream *decoder) {
    return decoder ? decoder->track_number : 0;
}

int64_t kog_vgmstream_render(KogVgmstream *decoder, float *output, size_t frames) {
    int result;
    if (!decoder || !output || frames == 0 || frames > INT_MAX) {
        return -1;
    }
    result = libvgmstream_fill(decoder->stream, output, (int)frames);
    if (result < 0) {
        return -1;
    }
    return decoder->stream->decoder->buf_samples;
}

uint64_t kog_vgmstream_seek(KogVgmstream *decoder, uint64_t frame) {
    int64_t actual;
    uint64_t total;
    if (!decoder) {
        return 0;
    }
    total = kog_vgmstream_total_frames(decoder);
    if (frame > total) {
        frame = total;
    }
    if (frame > INT64_MAX) {
        frame = INT64_MAX;
    }
    libvgmstream_seek(decoder->stream, (int64_t)frame);
    actual = libvgmstream_get_play_position(decoder->stream);
    return actual < 0 ? 0 : (uint64_t)actual;
}

int kog_vgmstream_supports_extension(const char *extension) {
    libvgmstream_valid_t config = {0};
    if (!extension || !extension[0]) {
        return 0;
    }
    config.is_extension = true;
    config.reject_extensionless = true;
    config.accept_common = false;
    return libvgmstream_is_valid(extension, &config);
}

size_t kog_vgmstream_extension_count(void) {
    int count = 0;
    libvgmstream_get_extensions(&count);
    return count > 0 ? (size_t)count : 0;
}

const char *kog_vgmstream_extension(size_t index) {
    int count = 0;
    const char **extensions = libvgmstream_get_extensions(&count);
    if (!extensions || index >= (size_t)count) {
        return NULL;
    }
    return extensions[index];
}

uint32_t kog_vgmstream_api_version(void) {
    return libvgmstream_get_version();
}
