/* Check extraction using the freshly built mobile libarchive, not the host's. */
#include <archive.h>
#include <archive_entry.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    const char *names[] = {"lzma.7z", "lzma2.7z", "bzip2.zip", "lzma.zip",
        "tone.wav.xz", "tone.wav.bz2", "zstd.tar", "lz4.tar"};
    unsigned char expected[44144], actual[44145];
    char path[4096];
    if (argc != 2) return 2;
    snprintf(path, sizeof(path), "%s/tone.wav", argv[1]);
    FILE *f = fopen(path, "rb");
    if (!f || fread(expected, 1, sizeof(expected), f) != sizeof(expected)) return 1;
    fclose(f);
    printf("%s\n", archive_version_details());
    for (unsigned i = 0; i < sizeof(names)/sizeof(names[0]); ++i) {
        struct archive *a = archive_read_new();
        struct archive_entry *entry = NULL;
        archive_read_support_filter_all(a);
        archive_read_support_format_all(a);
        archive_read_support_format_raw(a);
        snprintf(path, sizeof(path), "%s/%s", argv[1], names[i]);
        if (archive_read_open_filename(a, path, 10240) != ARCHIVE_OK ||
            archive_read_next_header(a, &entry) != ARCHIVE_OK) {
            fprintf(stderr, "%s: %s\n", names[i], archive_error_string(a)); return 1;
        }
        size_t size = 0;
        la_ssize_t n;
        while ((n = archive_read_data(a, actual + size, sizeof(actual) - size)) > 0) {
            size += n;
            if (size == sizeof(actual)) break;
        }
        if (n < 0 || size != sizeof(expected) || memcmp(actual, expected, size)) {
            fprintf(stderr, "%s: incorrect extracted PCM (%s)\n", names[i], archive_error_string(a)); return 1;
        }
        printf("%s: exact content\n", names[i]);
        archive_read_free(a);
    }
    return 0;
}
