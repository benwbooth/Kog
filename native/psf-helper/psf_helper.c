/*
 * Kog PSF helper process.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-2.0-only
 *
 * This program is deliberately a separate executable. It uses libupse under
 * GPL-2.0-only and communicates with the GPL-3.0-or-later Kog application over
 * a small documented byte-stream protocol; libupse is never linked into Kog.
 */

#include <errno.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#endif

#include <zlib.h>

#include "upse.h"

void kog_upse_disable_stop(upse_module_t *module);

#define KOG_PSF_MAX_FILE_BYTES (256U * 1024U * 1024U)
#define KOG_PSF_MAX_DECOMPRESSED_BYTES (32U * 1024U * 1024U)
#define KOG_PSF_RAM_BYTES (2U * 1024U * 1024U)
#define KOG_PSF_MAX_LIBRARY_DEPTH 16U
#define KOG_PSF_CHANNELS 2U
#define KOG_PSF_RATE 44100U

typedef struct
{
    uint8_t version;
    int has_program;
    int has_length;
    int has_fade;
    char length[256];
    char fade[256];
} validated_psf_t;

static int ascii_equal(const char *left, size_t left_length, const char *right)
{
    size_t index;
    const size_t right_length = strlen(right);
    if (left_length != right_length)
        return 0;
    for (index = 0; index < left_length; ++index)
    {
        unsigned char a = (unsigned char)left[index];
        unsigned char b = (unsigned char)right[index];
        if (a >= 'A' && a <= 'Z')
            a = (unsigned char)(a - 'A' + 'a');
        if (b >= 'A' && b <= 'Z')
            b = (unsigned char)(b - 'A' + 'a');
        if (a != b)
            return 0;
    }
    return 1;
}

#ifdef _WIN32
static wchar_t *utf8_to_wide(const char *text)
{
    int length;
    wchar_t *wide;
    if (text == NULL)
        return NULL;
    length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, NULL, 0);
    if (length <= 0)
        return NULL;
    wide = (wchar_t *)calloc((size_t)length, sizeof(*wide));
    if (wide == NULL)
        return NULL;
    if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, wide, length) == 0)
    {
        free(wide);
        return NULL;
    }
    return wide;
}

static char *wide_to_utf8(const wchar_t *text)
{
    int length;
    char *utf8;
    if (text == NULL)
        return NULL;
    length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text, -1, NULL, 0, NULL, NULL);
    if (length <= 0)
        return NULL;
    utf8 = (char *)calloc((size_t)length, 1);
    if (utf8 == NULL)
        return NULL;
    if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text, -1, utf8, length, NULL, NULL) == 0)
    {
        free(utf8);
        return NULL;
    }
    return utf8;
}
#endif

static void *open_utf8(const char *path, const char *mode)
{
#ifdef _WIN32
    wchar_t *wide_path = utf8_to_wide(path);
    wchar_t wide_mode[8];
    size_t index;
    FILE *file;
    if (wide_path == NULL || strlen(mode) >= sizeof(wide_mode) / sizeof(wide_mode[0]))
    {
        free(wide_path);
        return NULL;
    }
    for (index = 0; mode[index] != '\0'; ++index)
        wide_mode[index] = (wchar_t)(unsigned char)mode[index];
    wide_mode[index] = L'\0';
    file = _wfopen(wide_path, wide_mode);
    free(wide_path);
    return file;
#else
    return fopen(path, mode);
#endif
}

static size_t stdio_read(void *pointer, size_t size, size_t count, void *handle)
{
    return fread(pointer, size, count, (FILE *)handle);
}

static int stdio_seek(void *handle, long offset, int origin)
{
    return fseek((FILE *)handle, offset, origin);
}

static int stdio_close(void *handle)
{
    return fclose((FILE *)handle);
}

static long stdio_tell(void *handle)
{
    return ftell((FILE *)handle);
}

static const upse_iofuncs_t kog_io = {
    open_utf8, stdio_read, stdio_seek, stdio_close, stdio_tell};

static uint32_t read_u32_le(const uint8_t *bytes)
{
    return (uint32_t)bytes[0] | ((uint32_t)bytes[1] << 8) |
           ((uint32_t)bytes[2] << 16) | ((uint32_t)bytes[3] << 24);
}

static int load_file(const char *path, uint8_t **bytes, size_t *length)
{
    FILE *file = (FILE *)open_utf8(path, "rb");
    long file_length;
    uint8_t *buffer;
    if (file == NULL)
    {
        fprintf(stderr, "cannot open PSF dependency %s: %s\n", path, strerror(errno));
        return 0;
    }
    if (fseek(file, 0, SEEK_END) != 0 || (file_length = ftell(file)) < 0 ||
        file_length > (long)KOG_PSF_MAX_FILE_BYTES || fseek(file, 0, SEEK_SET) != 0)
    {
        fprintf(stderr, "PSF file has an unsupported size: %s\n", path);
        fclose(file);
        return 0;
    }
    buffer = (uint8_t *)malloc((size_t)file_length == 0 ? 1U : (size_t)file_length);
    if (buffer == NULL)
    {
        fprintf(stderr, "out of memory while reading %s\n", path);
        fclose(file);
        return 0;
    }
    if ((size_t)file_length != 0 && fread(buffer, 1, (size_t)file_length, file) != (size_t)file_length)
    {
        fprintf(stderr, "short read while validating %s\n", path);
        free(buffer);
        fclose(file);
        return 0;
    }
    fclose(file);
    *bytes = buffer;
    *length = (size_t)file_length;
    return 1;
}

static char *resolve_dependency(const char *base, const uint8_t *name, size_t name_length)
{
    const char *separator = strrchr(base, '/');
#ifdef _WIN32
    const char *backslash = strrchr(base, '\\');
    if (separator == NULL || (backslash != NULL && backslash > separator))
        separator = backslash;
#endif
    {
        const size_t prefix = separator == NULL ? 0U : (size_t)(separator - base + 1);
        char *resolved = (char *)malloc(prefix + name_length + 1U);
        if (resolved == NULL)
            return NULL;
        if (prefix != 0)
            memcpy(resolved, base, prefix);
        memcpy(resolved + prefix, name, name_length);
        resolved[prefix + name_length] = '\0';
        return resolved;
    }
}

static int validate_psf_recursive(const char *path,
                                  unsigned depth,
                                  uint8_t expected_version,
                                  validated_psf_t *validated)
{
    uint8_t *bytes = NULL;
    size_t length = 0;
    uint32_t reserved_length;
    uint32_t compressed_length;
    uint32_t expected_crc;
    uint64_t data_end;
    size_t tag_offset;
    int has_program = 0;

    if (depth > KOG_PSF_MAX_LIBRARY_DEPTH)
    {
        fprintf(stderr, "PSF library nesting exceeds %u levels\n", KOG_PSF_MAX_LIBRARY_DEPTH);
        return 0;
    }
    if (!load_file(path, &bytes, &length))
        return 0;
    if (length < 16 || memcmp(bytes, "PSF\x01", 4) != 0)
    {
        fprintf(stderr, "unsupported or truncated PSF file: %s\n", path);
        free(bytes);
        return 0;
    }
    if (expected_version != 0 && bytes[3] != expected_version)
    {
        fprintf(stderr, "PSF library version mismatch in %s\n", path);
        free(bytes);
        return 0;
    }

    reserved_length = read_u32_le(bytes + 4);
    compressed_length = read_u32_le(bytes + 8);
    expected_crc = read_u32_le(bytes + 12);
    data_end = 16ULL + (uint64_t)reserved_length + (uint64_t)compressed_length;
    if (data_end > (uint64_t)length || (bytes[3] == 1 && (reserved_length & 3U) != 0))
    {
        fprintf(stderr, "PSF section lengths are invalid in %s\n", path);
        free(bytes);
        return 0;
    }

    if (bytes[3] == 1 && compressed_length != 0)
    {
        const uint8_t *compressed = bytes + 16U + reserved_length;
        uint8_t *executable = (uint8_t *)malloc(KOG_PSF_MAX_DECOMPRESSED_BYTES);
        uLongf executable_length = KOG_PSF_MAX_DECOMPRESSED_BYTES;
        uint32_t address;
        size_t payload_length;
        if (executable == NULL)
        {
            fprintf(stderr, "out of memory while validating %s\n", path);
            free(bytes);
            return 0;
        }
        if ((uint32_t)crc32(0, compressed, compressed_length) != expected_crc ||
            uncompress(executable, &executable_length, compressed, compressed_length) != Z_OK)
        {
            fprintf(stderr, "invalid compressed PlayStation executable in %s\n", path);
            free(executable);
            free(bytes);
            return 0;
        }
        if (executable_length == 0)
        {
            free(executable);
            executable = NULL;
        }
        else if (executable_length < 2048U || memcmp(executable, "PS-X EXE", 8) != 0)
        {
            fprintf(stderr, "invalid compressed PlayStation executable in %s\n", path);
            free(executable);
            free(bytes);
            return 0;
        }
        if (executable == NULL)
            goto validated_executable;
        address = read_u32_le(executable + 0x18) & 0x1fffffffU;
        payload_length = (size_t)executable_length - 2048U;
        if ((address & 3U) != 0 || address >= KOG_PSF_RAM_BYTES ||
            payload_length > (size_t)(KOG_PSF_RAM_BYTES - address))
        {
            fprintf(stderr, "PlayStation executable exceeds emulated RAM in %s\n", path);
            free(executable);
            free(bytes);
            return 0;
        }
        free(executable);
        has_program = 1;
validated_executable:
        ;
    }
    tag_offset = (size_t)data_end;
    if (length - tag_offset >= 5U && memcmp(bytes + tag_offset, "[TAG]", 5) == 0)
    {
        size_t cursor = tag_offset + 5U;
        unsigned tag_count = 0;
        while (cursor < length)
        {
            size_t line_end = cursor;
            size_t equal;
            size_t value_end;
            while (line_end < length && bytes[line_end] != '\n')
                ++line_end;
            value_end = line_end;
            if (value_end > cursor && bytes[value_end - 1] == '\r')
                --value_end;
            equal = cursor;
            while (equal < value_end && bytes[equal] != '=')
                ++equal;
            if (value_end != cursor)
            {
                ++tag_count;
                if (tag_count > 32U || equal == value_end || equal - cursor > 255U ||
                    value_end - equal - 1U > 255U ||
                    memchr(bytes + cursor, '\0', value_end - cursor) != NULL)
                {
                    fprintf(stderr, "PSF tags exceed libupse's safe bounds in %s\n", path);
                    free(bytes);
                    return 0;
                }
                if (ascii_equal((const char *)bytes + cursor, equal - cursor, "_lib") ||
                    (equal - cursor >= 5U && bytes[cursor] == '_' &&
                     bytes[cursor + 1] == 'l' && bytes[cursor + 2] == 'i' &&
                     bytes[cursor + 3] == 'b' &&
                     bytes[cursor + 4] >= '2' && bytes[cursor + 4] <= '9'))
                {
                    const uint8_t *value = bytes + equal + 1U;
                    const size_t value_length = value_end - equal - 1U;
                    validated_psf_t dependency = {0};
                    char *resolved;
                    if (equal - cursor >= 5U && bytes[cursor + 4] == '9')
                    {
                        fprintf(stderr, "PSF _lib9 is unsafe in this libupse revision: %s\n", path);
                        free(bytes);
                        return 0;
                    }
                    if (value_length == 0)
                    {
                        fprintf(stderr, "empty PSF library tag in %s\n", path);
                        free(bytes);
                        return 0;
                    }
                    resolved = resolve_dependency(path, value, value_length);
                    if (resolved == NULL ||
                        !validate_psf_recursive(resolved, depth + 1U, bytes[3], &dependency))
                    {
                        free(resolved);
                        free(bytes);
                        return 0;
                    }
                    has_program = has_program || dependency.has_program;
                    free(resolved);
                }
                else if (ascii_equal((const char *)bytes + cursor, equal - cursor, "length"))
                {
                    const size_t value_length = value_end - equal - 1U;
                    memcpy(validated->length, bytes + equal + 1U, value_length);
                    validated->length[value_length] = '\0';
                    validated->has_length = 1;
                }
                else if (ascii_equal((const char *)bytes + cursor, equal - cursor, "fade"))
                {
                    const size_t value_length = value_end - equal - 1U;
                    memcpy(validated->fade, bytes + equal + 1U, value_length);
                    validated->fade[value_length] = '\0';
                    validated->has_fade = 1;
                }
            }
            else
            {
                fprintf(stderr, "blank PSF tag lines are unsafe in this libupse revision: %s\n", path);
                free(bytes);
                return 0;
            }
            cursor = line_end < length ? line_end + 1U : length;
        }
    }

    validated->version = bytes[3];
    validated->has_program = has_program;
    free(bytes);
    return 1;
}

static uint64_t frames_from_milliseconds(uint32_t milliseconds)
{
    return ((uint64_t)milliseconds * KOG_PSF_RATE) / 1000U;
}

static uint32_t parse_time_milliseconds(const char *text)
{
    const char *component = text;
    long double total_seconds = 0.0L;
    if (text == NULL || *text == '\0')
        return 0;
    for (;;)
    {
        char *end = NULL;
        long double value;
        errno = 0;
        value = strtold(component, &end);
        if (errno != 0 || end == component || value < 0.0L || !isfinite(value) ||
            (*end != ':' && *end != '\0'))
            return 0;
        total_seconds = total_seconds * 60.0L + value;
        if (*end == '\0')
            break;
        component = end + 1;
    }
    if (total_seconds <= 0.0L || total_seconds * 1000.0L > (long double)UINT32_MAX)
        return 0;
    return (uint32_t)(total_seconds * 1000.0L + 0.5L);
}

static int write_u32_le(uint32_t value)
{
    uint8_t bytes[4] = {(uint8_t)value, (uint8_t)(value >> 8),
                        (uint8_t)(value >> 16), (uint8_t)(value >> 24)};
    return fwrite(bytes, 1, sizeof(bytes), stdout) == sizeof(bytes);
}

static int write_u64_le(uint64_t value)
{
    uint8_t bytes[8];
    unsigned index;
    for (index = 0; index < 8U; ++index)
        bytes[index] = (uint8_t)(value >> (index * 8U));
    return fwrite(bytes, 1, sizeof(bytes), stdout) == sizeof(bytes);
}

static const char *metadata_text(const char *value)
{
    if (value == NULL || value[0] == '\0' || ascii_equal(value, strlen(value), "n/a"))
        return "";
    return value;
}

static const char *metadata_tag(const upse_xsf_t *xsf, const char *name)
{
    unsigned index;
    if (xsf == NULL)
        return "";
    for (index = 0; index < MAX_UNKNOWN_TAGS; ++index)
    {
        if (ascii_equal(xsf->tag_name[index], strlen(xsf->tag_name[index]), name))
            return metadata_text(xsf->tag_data[index]);
    }
    return "";
}

static int write_header(const upse_module_t *module,
                        uint8_t format_version,
                        uint64_t main_frames,
                        uint64_t total_frames)
{
    const char *fields[5];
    size_t lengths[5];
    unsigned index;
    static const uint8_t magic[8] = {'K', 'O', 'G', 'P', 'S', 'F', '1', 0};

    fields[0] = metadata_text(module->metadata->title);
    fields[1] = metadata_text(module->metadata->artist);
    fields[2] = metadata_text(module->metadata->game);
    fields[3] = metadata_tag(module->metadata->xsf, "genre");
    fields[4] = metadata_text(module->metadata->year);
    if (fields[4][0] == '\0')
        fields[4] = metadata_tag(module->metadata->xsf, "date");
    for (index = 0; index < 5U; ++index)
        lengths[index] = strlen(fields[index]);

    if (fwrite(magic, 1, sizeof(magic), stdout) != sizeof(magic) ||
        !write_u32_le(1U) || !write_u32_le(format_version) ||
        !write_u32_le(KOG_PSF_RATE) || !write_u32_le(KOG_PSF_CHANNELS) ||
        !write_u64_le(total_frames) || !write_u64_le(main_frames))
        return 0;
    for (index = 0; index < 5U; ++index)
    {
        if (lengths[index] > UINT32_MAX || !write_u32_le((uint32_t)lengths[index]))
            return 0;
    }
    for (index = 0; index < 5U; ++index)
    {
        if (lengths[index] != 0 && fwrite(fields[index], 1, lengths[index], stdout) != lengths[index])
            return 0;
    }
    return fflush(stdout) == 0;
}

static int write_samples_le(const s16 *samples, size_t frames)
{
    uint8_t bytes[4096];
    size_t frame = 0;
    while (frame < frames)
    {
        const size_t capacity = sizeof(bytes) / (KOG_PSF_CHANNELS * sizeof(int16_t));
        const size_t chunk = frames - frame < capacity ? frames - frame : capacity;
        size_t sample;
        for (sample = 0; sample < chunk * KOG_PSF_CHANNELS; ++sample)
        {
            const uint16_t value = (uint16_t)samples[frame * KOG_PSF_CHANNELS + sample];
            bytes[sample * 2U] = (uint8_t)value;
            bytes[sample * 2U + 1U] = (uint8_t)(value >> 8);
        }
        if (fwrite(bytes, KOG_PSF_CHANNELS * sizeof(int16_t), chunk, stdout) != chunk)
            return 0;
        frame += chunk;
    }
    return 1;
}

static int parse_u64(const char *text, uint64_t *value)
{
    char *end = NULL;
    unsigned long long parsed;
    errno = 0;
    parsed = strtoull(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0')
        return 0;
    *value = (uint64_t)parsed;
    return 1;
}

static int run_helper(const char *path,
                      const char *start_text,
                      const char *default_length_text,
                      const char *default_fade_text)
{
    validated_psf_t validated = {0};
    upse_module_t *module;
    uint64_t start_frame;
    uint64_t default_length;
    uint64_t default_fade;
    uint32_t length_ms;
    uint32_t fade_ms;
    uint64_t main_frames;
    uint64_t fade_frames;
    uint64_t total_frames;
    uint64_t generated_frames = 0;

    if (!parse_u64(start_text, &start_frame) || !parse_u64(default_length_text, &default_length) ||
        !parse_u64(default_fade_text, &default_fade) || default_length == 0 ||
        default_length > UINT32_MAX || default_fade > UINT32_MAX)
    {
        fprintf(stderr, "invalid Kog PSF helper arguments\n");
        return 2;
    }
    if (!validate_psf_recursive(path, 0, 0, &validated) || !validated.has_program)
    {
        if (!validated.has_program)
            fprintf(stderr, "PSF library chain contains no executable data\n");
        return 3;
    }

    upse_module_init();
    module = upse_module_open(path, &kog_io);
    if (module == NULL || module->metadata == NULL)
    {
        fprintf(stderr, "libupse rejected %s\n", path);
        upse_module_close(module);
        return 4;
    }

    length_ms = validated.has_length ? parse_time_milliseconds(validated.length) : 0;
    fade_ms = validated.has_fade ? parse_time_milliseconds(validated.fade) : 0;
    if (length_ms == 0)
    {
        length_ms = (uint32_t)default_length;
        fade_ms = (uint32_t)default_fade;
    }
    main_frames = frames_from_milliseconds(length_ms);
    fade_frames = frames_from_milliseconds(fade_ms);
    if (main_frames == 0 || UINT64_MAX - main_frames < fade_frames)
    {
        fprintf(stderr, "invalid PSF duration metadata\n");
        upse_module_close(module);
        return 5;
    }
    total_frames = main_frames + fade_frames;
    if (start_frame > total_frames)
        start_frame = total_frames;

    /* Kog applies one linear fade in the parent process. Disable libupse's
     * tag-derived stop/fade so defaults and tagged timing share one path. */
    kog_upse_disable_stop(module);

    if (!write_header(module, validated.version, main_frames, total_frames))
    {
        upse_module_close(module);
        return 0;
    }

    while (generated_frames < total_frames)
    {
        s16 *samples = NULL;
        const int rendered = upse_eventloop_render(module, &samples);
        size_t offset = 0;
        size_t available;
        uint64_t output_start;
        if (rendered <= 0 || samples == NULL)
        {
            if (generated_frames < total_frames)
            {
                fprintf(stderr, "libupse stopped at frame %llu of %llu\n",
                        (unsigned long long)generated_frames,
                        (unsigned long long)total_frames);
                upse_module_close(module);
                return 6;
            }
            break;
        }
        if (generated_frames < start_frame)
        {
            const uint64_t skipped = start_frame - generated_frames;
            offset = skipped < (uint64_t)rendered ? (size_t)skipped : (size_t)rendered;
        }
        output_start = generated_frames + offset;
        available = (size_t)rendered - offset;
        if (output_start < total_frames && available > (size_t)(total_frames - output_start))
            available = (size_t)(total_frames - output_start);
        if (available != 0 && !write_samples_le(samples + offset * KOG_PSF_CHANNELS, available))
        {
            upse_module_close(module);
            return 0;
        }
        generated_frames += (uint64_t)rendered;
    }

    upse_module_close(module);
    return 0;
}

#ifdef _WIN32
int wmain(int argc, wchar_t **argv)
{
    char *arguments[4] = {NULL, NULL, NULL, NULL};
    int index;
    int result;
    _setmode(_fileno(stdout), _O_BINARY);
    if (argc != 5)
    {
        fprintf(stderr, "usage: kog-psf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
        return 2;
    }
    for (index = 0; index < 4; ++index)
    {
        arguments[index] = wide_to_utf8(argv[index + 1]);
        if (arguments[index] == NULL)
        {
            while (index >= 0)
                free(arguments[index--]);
            fprintf(stderr, "invalid UTF-16 helper argument\n");
            return 2;
        }
    }
    result = run_helper(arguments[0], arguments[1], arguments[2], arguments[3]);
    for (index = 0; index < 4; ++index)
        free(arguments[index]);
    return result;
}
#else
int main(int argc, char **argv)
{
    if (argc != 5)
    {
        fprintf(stderr, "usage: kog-psf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS\n");
        return 2;
    }
    return run_helper(argv[1], argv[2], argv[3], argv[4]);
}
#endif
