#include "mt32emu_bridge.h"

#include <algorithm>
#include <cstring>
#include <filesystem>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <system_error>
#include <vector>

#include "c_interface/c_interface.h"

struct KogMt32
{
    mt32emu_context context = nullptr;
    uint32_t sampleRate = 0;
    std::string model;
    bool queueOverflow = false;

    ~KogMt32()
    {
        if(context != nullptr)
        {
            mt32emu_close_synth(context);
            mt32emu_free_context(context);
        }
    }
};

namespace fs = std::filesystem;

namespace
{
constexpr size_t MAX_ROM_FILES = 256;
constexpr uintmax_t MAX_ROM_BYTES = 16U * 1024U * 1024U;

mt32emu_report_handler_version MT32EMU_C_CALL reportHandlerVersion(mt32emu_report_handler_i)
{
    return MT32EMU_REPORT_HANDLER_VERSION_0;
}

mt32emu_boolean MT32EMU_C_CALL reportQueueOverflow(void *instanceData)
{
    static_cast<KogMt32 *>(instanceData)->queueOverflow = true;
    return MT32EMU_BOOL_FALSE;
}

const mt32emu_report_handler_i_v0 REPORT_HANDLER_V0 = {
    reportHandlerVersion,
    nullptr,
    nullptr,
    nullptr,
    nullptr,
    nullptr,
    reportQueueOverflow,
    nullptr,
    nullptr,
    nullptr,
    nullptr,
    nullptr,
    nullptr,
    nullptr,
    nullptr,
};

void copyError(char *target, size_t capacity, const std::string &message)
{
    if(target == nullptr || capacity == 0) return;
    const size_t length = std::min(capacity - 1, message.size());
    std::memcpy(target, message.data(), length);
    target[length] = '\0';
}

std::string returnCode(mt32emu_return_code code)
{
    switch(code)
    {
    case MT32EMU_RC_OK: return "success";
    case MT32EMU_RC_ROM_NOT_IDENTIFIED: return "ROM image was not recognized";
    case MT32EMU_RC_FILE_NOT_FOUND: return "ROM file was not found";
    case MT32EMU_RC_FILE_NOT_LOADED: return "ROM file could not be loaded";
    case MT32EMU_RC_MISSING_ROMS: return "a control or PCM ROM is missing";
    case MT32EMU_RC_NOT_OPENED: return "the synthesizer is not open";
    case MT32EMU_RC_QUEUE_FULL: return "the MIDI queue is full";
    case MT32EMU_RC_ROMS_NOT_PAIRABLE: return "the control and PCM ROMs are incompatible";
    case MT32EMU_RC_MACHINE_NOT_IDENTIFIED: return "the emulated machine was not recognized";
    default: return "Munt error " + std::to_string(static_cast<int>(code));
    }
}

std::vector<fs::path> romCandidates(const fs::path &directory)
{
    std::error_code error;
    if(!fs::is_directory(directory, error) || error)
        throw std::runtime_error("MT-32 ROM path is not a directory");

    std::vector<fs::path> candidates;
    for(fs::directory_iterator iterator(directory, error), end;
        !error && iterator != end;
        iterator.increment(error))
    {
        const fs::directory_entry &entry = *iterator;
        std::error_code entryError;
        if(!entry.is_regular_file(entryError) || entryError) continue;
        const uintmax_t size = entry.file_size(entryError);
        if(entryError || size == 0 || size > MAX_ROM_BYTES) continue;
        if(candidates.size() == MAX_ROM_FILES)
            throw std::runtime_error("MT-32 ROM directory contains more than 256 candidate files");
        candidates.push_back(entry.path());
    }
    if(error)
        throw std::runtime_error("reading MT-32 ROM directory failed: " + error.message());
    std::sort(candidates.begin(), candidates.end());
    return candidates;
}
} // namespace

extern "C" KogMt32 *kog_mt32_open(const char *rom_directory,
                                    uint32_t sample_rate,
                                    char *error,
                                    size_t error_size)
{
    try
    {
        if(rom_directory == nullptr || *rom_directory == '\0')
            throw std::runtime_error("MT-32 ROM directory is empty");
        if(sample_rate < 8000 || sample_rate > 192000)
            throw std::runtime_error("MT-32 output sample rate is outside Kog's supported range");

        std::unique_ptr<KogMt32> synth(new KogMt32());
        mt32emu_report_handler_i reportHandler;
        reportHandler.v0 = &REPORT_HANDLER_V0;
        synth->context = mt32emu_create_context(reportHandler, synth.get());
        if(synth->context == nullptr)
            throw std::runtime_error("creating the Munt emulation context failed");

        mt32emu_return_code openResult = MT32EMU_RC_MISSING_ROMS;
        for(const fs::path &path : romCandidates(fs::u8path(rom_directory)))
        {
            const std::string filename = path.u8string();
            const mt32emu_return_code addResult =
                mt32emu_add_rom_file(synth->context, filename.c_str());
            if(addResult < 0 && addResult != MT32EMU_RC_ROM_NOT_IDENTIFIED) continue;

            mt32emu_rom_info info {};
            mt32emu_get_rom_info(synth->context, &info);
            if(info.control_rom_id == nullptr || info.pcm_rom_id == nullptr) continue;

            mt32emu_select_renderer_type(synth->context, MT32EMU_RT_FLOAT);
            mt32emu_set_analog_output_mode(synth->context, MT32EMU_AOM_COARSE);
            mt32emu_set_stereo_output_samplerate(synth->context, sample_rate);
            openResult = mt32emu_open_synth(synth->context);
            if(openResult == MT32EMU_RC_OK)
            {
                synth->sampleRate = mt32emu_get_actual_stereo_output_samplerate(synth->context);
                synth->model = info.control_rom_description != nullptr
                    ? info.control_rom_description
                    : info.control_rom_id;
                static const uint8_t reset[] = {
                    0xF0, 0x41, 0x10, 0x16, 0x12, 0x7F, 0x00, 0x00, 0x01, 0xF7,
                };
                const mt32emu_return_code resetResult =
                    mt32emu_play_sysex(synth->context, reset, sizeof(reset));
                if(resetResult != MT32EMU_RC_OK)
                    throw std::runtime_error("queuing the MT-32 reset failed: " +
                                             returnCode(resetResult));
                return synth.release();
            }
        }
        throw std::runtime_error(
            "no complete compatible MT-32/CM-32L ROM pair was recognized (" +
            returnCode(openResult) + ")");
    }
    catch(const std::exception &exception)
    {
        copyError(error, error_size, exception.what());
        return nullptr;
    }
}

extern "C" void kog_mt32_free(KogMt32 *synth)
{
    delete synth;
}

extern "C" const char *kog_mt32_model(const KogMt32 *synth)
{
    return synth == nullptr ? "" : synth->model.c_str();
}

extern "C" uint32_t kog_mt32_sample_rate(const KogMt32 *synth)
{
    return synth == nullptr ? 0 : synth->sampleRate;
}

extern "C" int kog_mt32_send(KogMt32 *synth,
                              const uint8_t *bytes,
                              size_t length,
                              char *error,
                              size_t error_size)
{
    if(synth == nullptr || synth->context == nullptr || bytes == nullptr || length == 0 ||
       length > std::numeric_limits<mt32emu_bit32u>::max())
    {
        copyError(error, error_size, "invalid MT-32 MIDI event");
        return 0;
    }
    synth->queueOverflow = false;
    mt32emu_parse_stream(synth->context, bytes, static_cast<mt32emu_bit32u>(length));
    if(synth->queueOverflow)
    {
        copyError(error, error_size, "the MT-32 MIDI queue is full");
        return 0;
    }
    return 1;
}

extern "C" int kog_mt32_render(KogMt32 *synth,
                                float *output,
                                size_t frames,
                                char *error,
                                size_t error_size)
{
    if(synth == nullptr || synth->context == nullptr || output == nullptr || frames == 0 ||
       frames > std::numeric_limits<mt32emu_bit32u>::max())
    {
        copyError(error, error_size, "invalid MT-32 render request");
        return 0;
    }
    mt32emu_render_float(synth->context, output, static_cast<mt32emu_bit32u>(frames));
    return 1;
}
