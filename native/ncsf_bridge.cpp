#include "ncsf_bridge.h"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <exception>
#include <filesystem>
#include <fstream>
#include <limits>
#include <memory>
#include <mutex>
#include <new>
#include <stdexcept>
#include <string>
#include <vector>

#include "Player.h"
#include "SDAT.h"
#include "common.h"
#include "psflib.h"

namespace {

constexpr uint8_t ncsf_version = 0x25;
constexpr uint32_t sample_rate = 44100;
constexpr uint32_t channels = 2;
constexpr size_t maximum_sdat_size = 512u * 1024u * 1024u;

thread_local std::string last_error;
std::once_flag player_warmup_once;

struct PsfFile {
    explicit PsfFile(const char *path)
        : stream(std::filesystem::u8path(path), std::ios::binary) {}

    std::ifstream stream;
};

struct LoaderState {
    uint32_t sseq = 0;
    std::vector<uint8_t> sdat_data;
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

struct FileRange {
    size_t offset;
    size_t size;
};

struct WaveReference {
    uint16_t archive;
    uint16_t sample;
};

[[noreturn]] void malformed(const std::string &detail) {
    throw std::runtime_error("malformed NCSF SDAT: " + detail);
}

void require_range(const std::vector<uint8_t> &data,
                   size_t offset,
                   size_t size,
                   const std::string &label) {
    if (offset > data.size() || size > data.size() - offset) {
        malformed(label + " is outside the SDAT image");
    }
}

uint8_t checked_u8(const std::vector<uint8_t> &data,
                   size_t offset,
                   const std::string &label) {
    require_range(data, offset, 1, label);
    return data[offset];
}

uint16_t checked_u16(const std::vector<uint8_t> &data,
                     size_t offset,
                     const std::string &label) {
    require_range(data, offset, 2, label);
    return static_cast<uint16_t>(data[offset]) |
           (static_cast<uint16_t>(data[offset + 1]) << 8);
}

uint32_t checked_u32(const std::vector<uint8_t> &data,
                     size_t offset,
                     const std::string &label) {
    require_range(data, offset, 4, label);
    return static_cast<uint32_t>(data[offset]) |
           (static_cast<uint32_t>(data[offset + 1]) << 8) |
           (static_cast<uint32_t>(data[offset + 2]) << 16) |
           (static_cast<uint32_t>(data[offset + 3]) << 24);
}

void require_tag(const std::vector<uint8_t> &data,
                 size_t offset,
                 const char (&tag)[5],
                 const std::string &label) {
    require_range(data, offset, 4, label);
    if (!std::equal(data.begin() + offset, data.begin() + offset + 4, tag)) {
        malformed(label + " has an invalid signature");
    }
}

size_t checked_product(size_t left, size_t right, const std::string &label) {
    if (left != 0 && right > std::numeric_limits<size_t>::max() / left) {
        malformed(label + " is too large");
    }
    return left * right;
}

FileRange validate_section(const std::vector<uint8_t> &data,
                           size_t offset,
                           size_t table_size,
                           const char (&tag)[5],
                           size_t minimum_size,
                           const std::string &label) {
    require_range(data, offset, table_size, label);
    require_tag(data, offset, tag, label);
    const size_t declared_size = checked_u32(data, offset + 4, label + " size");
    if (declared_size < minimum_size || declared_size > table_size) {
        malformed(label + " has an invalid declared size");
    }
    return {offset, declared_size};
}

void validate_nds_file(const std::vector<uint8_t> &data,
                       const FileRange &file,
                       const char (&tag)[5],
                       const std::string &label) {
    require_range(data, file.offset, file.size, label);
    if (file.size < 16) {
        malformed(label + " is shorter than an NDS header");
    }
    require_tag(data, file.offset, tag, label);
    if (checked_u32(data, file.offset + 4, label + " byte-order marker") != 0x0100FEFF) {
        malformed(label + " has an invalid byte-order marker");
    }
    const size_t declared_size = checked_u32(data, file.offset + 8, label + " file size");
    if (declared_size < 16 || declared_size > file.size) {
        malformed(label + " has an invalid file size");
    }
}

size_t info_entry(const std::vector<uint8_t> &data,
                  const FileRange &info,
                  size_t record_index,
                  size_t entry_index,
                  size_t entry_size,
                  const std::string &label) {
    const size_t record_offset_slot = info.offset + 8 + record_index * 4;
    const size_t record_relative = checked_u32(data, record_offset_slot, label + " record offset");
    if (record_relative == 0 || record_relative >= info.size) {
        malformed(label + " record is missing");
    }
    const size_t record = info.offset + record_relative;
    const size_t count = checked_u32(data, record, label + " count");
    if (entry_index >= count) {
        malformed(label + " index is outside its INFO record");
    }
    const size_t offsets_size = checked_product(count, 4, label + " offsets");
    require_range(data, record + 4, offsets_size, label + " offsets");
    if (record + 4 + offsets_size > info.offset + info.size) {
        malformed(label + " offsets exceed the INFO section");
    }
    const size_t relative = checked_u32(data, record + 4 + entry_index * 4, label + " offset");
    if (relative == 0 || relative >= info.size) {
        malformed(label + " entry is missing");
    }
    const size_t entry = info.offset + relative;
    require_range(data, entry, entry_size, label + " entry");
    if (entry + entry_size > info.offset + info.size) {
        malformed(label + " entry exceeds the INFO section");
    }
    return entry;
}

FileRange fat_file(const std::vector<uint8_t> &data,
                   const FileRange &fat,
                   size_t file_id,
                   const std::string &label) {
    const size_t count = checked_u32(data, fat.offset + 8, "FAT record count");
    if (file_id >= count) {
        malformed(label + " file ID is outside the FAT");
    }
    const size_t records_size = checked_product(count, 16, "FAT records");
    if (records_size > fat.size - 12) {
        malformed("FAT records exceed the FAT section");
    }
    const size_t record = fat.offset + 12 + file_id * 16;
    const size_t offset = checked_u32(data, record, label + " file offset");
    const size_t size = checked_u32(data, record + 4, label + " file size");
    require_range(data, offset, size, label + " file");
    return {offset, size};
}

void validate_sseq(const std::vector<uint8_t> &data, const FileRange &file) {
    validate_nds_file(data, file, "SSEQ", "SSEQ");
    require_tag(data, file.offset + 16, "DATA", "SSEQ DATA");
    const size_t section_size = checked_u32(data, file.offset + 20, "SSEQ DATA size");
    const size_t data_offset = checked_u32(data, file.offset + 24, "SSEQ data offset");
    if (section_size < 12 || section_size > file.size - 16 || data_offset >= file.size) {
        malformed("SSEQ DATA layout is invalid");
    }
    const size_t sequence_size = section_size - 12;
    if (sequence_size == 0 || sequence_size > file.size - data_offset) {
        malformed("SSEQ command stream is empty or truncated");
    }
}

std::vector<WaveReference> validate_sbnk(const std::vector<uint8_t> &data,
                                         const FileRange &file) {
    validate_nds_file(data, file, "SBNK", "SBNK");
    require_tag(data, file.offset + 16, "DATA", "SBNK DATA");
    const size_t section_size = checked_u32(data, file.offset + 20, "SBNK DATA size");
    if (section_size < 44 || section_size > file.size - 16) {
        malformed("SBNK DATA layout is invalid");
    }
    const size_t count = checked_u32(data, file.offset + 56, "SBNK instrument count");
    const size_t table_size = checked_product(count, 4, "SBNK instrument table");
    if (table_size > file.size - 60) {
        malformed("SBNK instrument table is truncated");
    }

    std::vector<WaveReference> references;
    const auto validate_leaf = [&](size_t offset, uint16_t type) {
        if (type == 0) {
            return;
        }
        if (type > 3) {
            malformed("SBNK uses an unsupported leaf instrument type");
        }
        require_range(data, offset, 10, "SBNK instrument range");
        if (offset + 10 > file.offset + file.size) {
            malformed("SBNK instrument range exceeds its file");
        }
        if (type == 1) {
            references.push_back({checked_u16(data, offset + 2, "SBNK wave archive"),
                                  checked_u16(data, offset, "SBNK wave sample")});
        }
    };

    for (size_t index = 0; index < count; ++index) {
        const size_t record = file.offset + 60 + index * 4;
        const uint8_t type = checked_u8(data, record, "SBNK instrument type");
        if (type == 0) {
            continue;
        }
        const size_t relative = checked_u16(data, record + 1, "SBNK instrument offset");
        if (relative >= file.size) {
            malformed("SBNK instrument offset exceeds its file");
        }
        size_t range = file.offset + relative;
        if (type == 16) {
            const uint8_t low = checked_u8(data, range, "SBNK low note");
            const uint8_t high = checked_u8(data, range + 1, "SBNK high note");
            if (high < low) {
                malformed("SBNK split instrument has an inverted note range");
            }
            range += 2;
            for (size_t note = low; note <= high; ++note) {
                const uint16_t leaf = checked_u16(data, range, "SBNK split instrument type");
                validate_leaf(range + 2, leaf);
                range += 12;
            }
        } else if (type == 17) {
            require_range(data, range, 8, "SBNK regional instrument boundaries");
            size_t region_count = 0;
            while (region_count < 8 && data[range + region_count] != 0) {
                ++region_count;
            }
            range += 8;
            for (size_t region = 0; region < region_count; ++region) {
                const uint16_t leaf = checked_u16(data, range, "SBNK regional instrument type");
                validate_leaf(range + 2, leaf);
                range += 12;
            }
        } else {
            validate_leaf(range, type);
        }
    }
    return references;
}

std::vector<bool> validate_swar(const std::vector<uint8_t> &data, const FileRange &file) {
    validate_nds_file(data, file, "SWAR", "SWAR");
    require_tag(data, file.offset + 16, "DATA", "SWAR DATA");
    const size_t section_size = checked_u32(data, file.offset + 20, "SWAR DATA size");
    if (section_size < 44 || section_size > file.size - 16) {
        malformed("SWAR DATA layout is invalid");
    }
    const size_t count = checked_u32(data, file.offset + 56, "SWAR sample count");
    const size_t offsets_size = checked_product(count, 4, "SWAR sample offsets");
    if (offsets_size > file.size - 60) {
        malformed("SWAR sample table is truncated");
    }
    std::vector<bool> present(count, false);
    for (size_t index = 0; index < count; ++index) {
        const size_t relative = checked_u32(data, file.offset + 60 + index * 4,
                                            "SWAR sample offset");
        if (relative == 0) {
            continue;
        }
        present[index] = true;
        if (relative >= file.size) {
            malformed("SWAR sample offset exceeds its file");
        }
        const size_t sample = file.offset + relative;
        const uint8_t wave_type = checked_u8(data, sample, "SWAV type");
        if (wave_type > 2) {
            malformed("SWAV has an unsupported encoding");
        }
        const size_t loop = checked_u16(data, sample + 6, "SWAV loop length");
        const size_t non_loop = checked_u32(data, sample + 8, "SWAV non-loop length");
        if (loop > std::numeric_limits<size_t>::max() - non_loop) {
            malformed("SWAV sample length is too large");
        }
        const size_t encoded_size = checked_product(loop + non_loop, 4, "SWAV sample data");
        if (encoded_size == 0 || (wave_type == 2 && encoded_size < 4)) {
            malformed("SWAV sample data is empty");
        }
        require_range(data, sample + 12, encoded_size, "SWAV sample data");
        if (sample + 12 + encoded_size > file.offset + file.size) {
            malformed("SWAV sample data exceeds its file");
        }
    }
    return present;
}

void validate_sdat(const std::vector<uint8_t> &data, uint32_t selected_sequence) {
    const FileRange whole{0, data.size()};
    validate_nds_file(data, whole, "SDAT", "SDAT");
    if (data.size() < 48) {
        malformed("SDAT section table is truncated");
    }
    const size_t declared_size = checked_u32(data, 8, "SDAT file size");
    if (declared_size != data.size()) {
        malformed("SDAT program size does not match its header");
    }
    const size_t info_offset = checked_u32(data, 24, "INFO offset");
    const size_t info_size = checked_u32(data, 28, "INFO size");
    const size_t fat_offset = checked_u32(data, 32, "FAT offset");
    const size_t fat_size = checked_u32(data, 36, "FAT size");
    const FileRange info = validate_section(data, info_offset, info_size, "INFO", 40, "INFO");
    const FileRange fat = validate_section(data, fat_offset, fat_size, "FAT ", 12, "FAT");

    const size_t sequence_entry = info_entry(data, info, 0, selected_sequence, 10, "SSEQ");
    const size_t sequence_file_id = checked_u16(data, sequence_entry, "SSEQ file ID");
    const size_t bank_id = checked_u16(data, sequence_entry + 4, "SSEQ bank ID");
    const size_t bank_entry = info_entry(data, info, 2, bank_id, 12, "SBNK");
    const size_t bank_file_id = checked_u16(data, bank_entry, "SBNK file ID");

    validate_sseq(data, fat_file(data, fat, sequence_file_id, "SSEQ"));
    const std::vector<WaveReference> references =
        validate_sbnk(data, fat_file(data, fat, bank_file_id, "SBNK"));

    std::vector<bool> wave_samples[4];
    bool loaded[4] = {false, false, false, false};
    for (size_t slot = 0; slot < 4; ++slot) {
        const uint16_t wave_id = checked_u16(data, bank_entry + 4 + slot * 2,
                                             "SBNK wave archive ID");
        if (wave_id == std::numeric_limits<uint16_t>::max()) {
            continue;
        }
        const size_t wave_entry = info_entry(data, info, 3, wave_id, 2, "SWAR");
        const size_t wave_file_id = checked_u16(data, wave_entry, "SWAR file ID");
        wave_samples[slot] = validate_swar(data, fat_file(data, fat, wave_file_id, "SWAR"));
        loaded[slot] = true;
    }
    for (const WaveReference &reference : references) {
        if (reference.archive >= 4 || !loaded[reference.archive] ||
            reference.sample >= wave_samples[reference.archive].size() ||
            !wave_samples[reference.archive][reference.sample]) {
            malformed("SBNK references a missing SWAR sample");
        }
    }
}

void set_error(const std::string &message) {
    last_error = message;
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

int load_ncsf(void *context,
              const uint8_t *executable,
              size_t executable_size,
              const uint8_t *reserved,
              size_t reserved_size) {
    LoaderState *state = static_cast<LoaderState *>(context);
    if (reserved_size >= 4) {
        state->sseq = static_cast<uint32_t>(reserved[0]) |
                      (static_cast<uint32_t>(reserved[1]) << 8) |
                      (static_cast<uint32_t>(reserved[2]) << 16) |
                      (static_cast<uint32_t>(reserved[3]) << 24);
    }
    if (executable_size < 12) {
        return 0;
    }
    const uint32_t sdat_size = static_cast<uint32_t>(executable[8]) |
                               (static_cast<uint32_t>(executable[9]) << 8) |
                               (static_cast<uint32_t>(executable[10]) << 16) |
                               (static_cast<uint32_t>(executable[11]) << 24);
    if (sdat_size < 16 || sdat_size > executable_size || sdat_size > maximum_sdat_size) {
        return -1;
    }
    if (state->sdat_data.size() < sdat_size) {
        state->sdat_data.resize(sdat_size);
    }
    std::copy_n(executable, sdat_size, state->sdat_data.begin());
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

std::unique_ptr<Player> make_player(const SDAT &sdat) {
    std::call_once(player_warmup_once, [] {
        Player warmup;
    });
    std::unique_ptr<Player> player(new Player());
    player->interpolation = INTERPOLATION_SINC;
    player->sampleRate = sample_rate;
    if (!player->Setup(sdat.sseq.get())) {
        throw std::runtime_error("SSEQPlayer could not allocate the initial track");
    }
    player->Timer();
    return player;
}

uint64_t frames_from_milliseconds(uint64_t milliseconds) {
    if (milliseconds > std::numeric_limits<uint64_t>::max() / sample_rate) {
        throw std::overflow_error("NCSF duration exceeds Kog's frame limit");
    }
    return (milliseconds * sample_rate + 999) / 1000;
}

} // namespace

struct KogNcsf {
    std::vector<uint8_t> sdat_data;
    std::unique_ptr<SDAT> sdat;
    std::unique_ptr<Player> player;
    MetadataState metadata;
    uint64_t main_frames = 0;
    uint64_t fade_frames = 0;
    uint64_t rendered_frames = 0;
    std::vector<uint8_t> native_samples;
};

extern "C" KogNcsf *kog_ncsf_open(const char *path,
                                     uint32_t default_length_milliseconds,
                                     uint32_t default_fade_milliseconds) {
    last_error.clear();
    if (path == nullptr || *path == '\0' || default_length_milliseconds == 0) {
        set_error("invalid NCSF open arguments");
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
                                    ncsf_version,
                                    load_ncsf,
                                    &loader,
                                    load_metadata,
                                    &metadata,
                                    0,
                                    psf_status,
                                    &status);
        if (result != ncsf_version) {
            set_error(status.empty() ? "psflib rejected the NCSF file" : status);
            return nullptr;
        }
        if (loader.sdat_data.size() < 16) {
            set_error("NCSF library chain contains no SDAT program");
            return nullptr;
        }
        if (!std::equal(loader.sdat_data.begin(), loader.sdat_data.begin() + 4, "SDAT")) {
            set_error("NCSF program does not contain an SDAT image");
            return nullptr;
        }
        validate_sdat(loader.sdat_data, loader.sseq);

        std::unique_ptr<KogNcsf> decoder(new KogNcsf());
        decoder->sdat_data = std::move(loader.sdat_data);
        PseudoFile file;
        file.data = &decoder->sdat_data;
        decoder->sdat = std::make_unique<SDAT>(file, loader.sseq);
        decoder->player = make_player(*decoder->sdat);
        decoder->metadata = std::move(metadata);

        uint64_t length_milliseconds = decoder->metadata.length_milliseconds;
        uint64_t fade_milliseconds = decoder->metadata.fade_milliseconds;
        if (length_milliseconds == 0) {
            length_milliseconds = default_length_milliseconds;
            fade_milliseconds = default_fade_milliseconds;
        }
        decoder->main_frames = frames_from_milliseconds(length_milliseconds);
        decoder->fade_frames = frames_from_milliseconds(fade_milliseconds);
        if (decoder->main_frames > std::numeric_limits<uint64_t>::max() - decoder->fade_frames) {
            set_error("NCSF duration and fade exceed Kog's frame limit");
            return nullptr;
        }
        return decoder.release();
    } catch (const std::exception &error) {
        set_error(error.what());
        return nullptr;
    } catch (...) {
        set_error("unknown NCSF initialization failure");
        return nullptr;
    }
}

extern "C" void kog_ncsf_free(KogNcsf *decoder) {
    delete decoder;
}

extern "C" uint32_t kog_ncsf_sample_rate(const KogNcsf *decoder) {
    return decoder == nullptr ? 0 : sample_rate;
}

extern "C" uint32_t kog_ncsf_channels(const KogNcsf *decoder) {
    return decoder == nullptr ? 0 : channels;
}

extern "C" uint64_t kog_ncsf_total_frames(const KogNcsf *decoder) {
    return decoder == nullptr ? 0 : decoder->main_frames + decoder->fade_frames;
}

extern "C" const char *kog_ncsf_title(const KogNcsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.title.c_str();
}

extern "C" const char *kog_ncsf_artist(const KogNcsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.artist.c_str();
}

extern "C" const char *kog_ncsf_album(const KogNcsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.album.c_str();
}

extern "C" const char *kog_ncsf_genre(const KogNcsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.genre.c_str();
}

extern "C" const char *kog_ncsf_date(const KogNcsf *decoder) {
    return decoder == nullptr ? "" : decoder->metadata.date.c_str();
}

extern "C" int64_t kog_ncsf_render(KogNcsf *decoder, float *output, size_t frames) {
    last_error.clear();
    if (decoder == nullptr || (output == nullptr && frames != 0)) {
        set_error("invalid NCSF render arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t remaining = total_frames - std::min(total_frames, decoder->rendered_frames);
    const size_t requested = static_cast<size_t>(std::min<uint64_t>(remaining, frames));
    if (requested == 0) {
        return 0;
    }
    if (requested > std::numeric_limits<unsigned>::max() ||
        requested > std::numeric_limits<size_t>::max() / (sizeof(int16_t) * channels)) {
        set_error("NCSF render request exceeds SSEQPlayer's limit");
        return -1;
    }

    try {
        decoder->native_samples.resize(requested * sizeof(int16_t) * channels);
        decoder->player->GenerateSamples(
            decoder->native_samples, 0, static_cast<unsigned>(requested));
        for (size_t frame = 0; frame < requested; ++frame) {
            float gain = 1.0f;
            const uint64_t absolute_frame = decoder->rendered_frames + frame;
            if (decoder->fade_frames != 0 && absolute_frame >= decoder->main_frames) {
                gain = static_cast<float>(total_frames - absolute_frame) /
                       static_cast<float>(decoder->fade_frames);
            }
            for (size_t channel = 0; channel < channels; ++channel) {
                const size_t index = frame * channels + channel;
                const size_t byte = index * sizeof(int16_t);
                const uint16_t encoded = static_cast<uint16_t>(decoder->native_samples[byte]) |
                                         (static_cast<uint16_t>(decoder->native_samples[byte + 1]) << 8);
                const int16_t sample = static_cast<int16_t>(encoded);
                output[index] = static_cast<float>(sample) * (gain / 32768.0f);
            }
        }
        decoder->rendered_frames += requested;
        return static_cast<int64_t>(requested);
    } catch (const std::exception &error) {
        set_error(error.what());
        return -1;
    } catch (...) {
        set_error("unknown NCSF rendering failure");
        return -1;
    }
}

extern "C" int64_t kog_ncsf_seek(KogNcsf *decoder, uint64_t frame) {
    last_error.clear();
    if (decoder == nullptr) {
        set_error("invalid NCSF seek arguments");
        return -1;
    }
    const uint64_t total_frames = decoder->main_frames + decoder->fade_frames;
    const uint64_t target = std::min(frame, total_frames);
    if (target == total_frames) {
        decoder->rendered_frames = target;
        return static_cast<int64_t>(target);
    }

    try {
        decoder->player = make_player(*decoder->sdat);
        decoder->rendered_frames = 0;
        std::vector<uint8_t> discard(2048 * sizeof(int16_t) * channels);
        while (decoder->rendered_frames < target) {
            const unsigned chunk = static_cast<unsigned>(
                std::min<uint64_t>(2048, target - decoder->rendered_frames));
            decoder->player->GenerateSamples(discard, 0, chunk);
            decoder->rendered_frames += chunk;
        }
        return static_cast<int64_t>(target);
    } catch (const std::exception &error) {
        set_error(error.what());
        return -1;
    } catch (...) {
        set_error("unknown NCSF seek failure");
        return -1;
    }
}

extern "C" const char *kog_ncsf_last_error(void) {
    return last_error.c_str();
}
