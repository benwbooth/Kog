// SPDX-License-Identifier: GPL-3.0-or-later
// In-process SFM decoder. The descriptor is private to this renderer instance.
#include "../embedded_stream.h"
#include "../inspection_snes.h"
#include "Spc_Sfm.h"
#include "Bml_Parser.h"
#include "Data_Reader.h"
#include <algorithm>
#include <array>
#include <charconv>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <memory>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

namespace {
constexpr uint64_t max_ms = 12 * 60 * 60 * 1000;
constexpr uint32_t rate = 32000;
struct CloseFile { void operator()(FILE* file) const { std::fclose(file); } };
using File = std::unique_ptr<FILE, CloseFile>;
void check(const char* failure) { if(failure) throw std::runtime_error(failure); }
struct Output {
  FILE* file;
  void bytes(const void* data, size_t size) {
    if(std::fwrite(data, 1, size, file) != size) throw std::runtime_error("SFM stream closed");
  }
  void number(uint64_t value, unsigned size) {
    uint8_t data[8];
    for(unsigned i = 0; i < size; ++i) { data[i] = value & 255; value >>= 8; }
    bytes(data, size);
  }
  void text(const char* value) {
    const size_t length = value ? std::strlen(value) : 0;
    if(length > 65535) throw std::runtime_error("SFM text exceeds 64 KiB");
    number(length, 4);
    if(length) bytes(value, length);
  }
};
void range(const Bml_Parser& metadata, const std::string& key, int64_t low, int64_t high) {
  const char* raw = metadata.enumValue(key);
  if(!raw) return;
  std::string_view text(raw);
  while(!text.empty() && text.front() <= ' ') text.remove_prefix(1);
  while(!text.empty() && text.back() <= ' ') text.remove_suffix(1);
  int64_t number = 0;
  auto parsed = std::from_chars(text.data(), text.data() + text.size(), number);
  if(parsed.ec != std::errc() || parsed.ptr != text.data() + text.size() || number < low || number > high)
    throw std::runtime_error("Invalid SFM metadata: " + key);
}
std::vector<uint8_t> load(const char* path) {
  const auto file = std::filesystem::u8path(path);
  auto size = std::filesystem::file_size(file);
  if(size < 8 + 65536 + 128 || size > 256 * 1024 * 1024)
    throw std::runtime_error("SFM file is truncated or exceeds 256 MiB");
  std::vector<uint8_t> data(static_cast<size_t>(size));
  std::ifstream input(file, std::ios::binary);
  if(!input.read(reinterpret_cast<char*>(data.data()), static_cast<std::streamsize>(size)))
    throw std::runtime_error("Cannot read SFM file");
  if(std::memcmp(data.data(), "SFM1", 4)) throw std::runtime_error("Invalid SFM signature");
  uint32_t length = 0;
  for(unsigned i = 0; i < 4; ++i) length |= uint32_t(data[4 + i]) << (8 * i);
  if(length > 4 * 1024 * 1024) throw std::runtime_error("SFM metadata exceeds 4 MiB");
  if(uint64_t(length) + 8 + 65536 + 128 > size) throw std::runtime_error("SFM state is truncated");
  // BML's tree owns children recursively. Limit nesting and allocation count
  // before either the validation parser or the snapshot parser sees the text.
  unsigned lines = 0, indent = 0;
  bool start_of_line = true;
  for(size_t i = 8; i < 8 + length; ++i) {
    const auto c = data[i];
    if(c == '\n') {
      if(++lines > 65536) throw std::runtime_error("Too many SFM metadata lines");
      start_of_line = true;
      indent = 0;
    } else if(start_of_line && c <= ' ') {
      if(++indent > 128) throw std::runtime_error("SFM metadata nesting exceeds 128");
    } else start_of_line = false;
  }
  Bml_Parser metadata;
  metadata.parseDocument(reinterpret_cast<const char*>(data.data() + 8), length);
  range(metadata, "timing:loopstart", 0, static_cast<int64_t>(size - length - 8 - 65536 - 128));
  range(metadata, "dsp:echohistaddr", 0, 7);
  range(metadata, "dsp:sample", 0, 31);
  range(metadata, "dsp:clock", -768, 768);
  for(unsigned voice = 0; voice < 8; ++voice) {
    const auto prefix = "dsp:voice[" + std::to_string(voice) + "]:";
    range(metadata, prefix + "brrhistaddr", 0, 11);
    range(metadata, prefix + "vidx", 0, 118);
    range(metadata, prefix + "envmode", 0, 3);
  }
  return data;
}
uint64_t milliseconds(long value, uint64_t fallback) {
  const auto result = value < 0 ? fallback : static_cast<uint64_t>(value);
  if(result > max_ms) throw std::runtime_error("SFM duration exceeds 12 hours");
  return result;
}
void render(const char* path, uint64_t start, uint32_t default_length, uint32_t default_fade, FILE* file) {
  auto data = load(path);
  Sfm_Emu emulator;
  check(emulator.set_sample_rate(rate));
  Mem_File_Reader reader(data.data(), static_cast<long>(data.size()));
  check(emulator.load(reader));
  track_info_t info {};
  check(emulator.track_info(&info, 0));
  auto length = milliseconds(info.length, default_length);
  if(info.length <= 0) {
    length = info.loop_length > 0 ? milliseconds(info.intro_length, 0) + 2 * milliseconds(info.loop_length, 0) : default_length;
  }
  const auto fade = milliseconds(info.fade_length, default_fade);
  if(length == 0 || length > max_ms || fade > max_ms - length) throw std::runtime_error("Invalid SFM duration");
  const auto main_frames = length * rate / 1000;
  const auto frames = (length + fade) * rate / 1000;
  start = std::min(start, frames);
  check(emulator.start_track(0));
  auto* smp = emulator.get_smp();
  smp->inspected_frames = static_cast<uint64_t>(emulator.inspection_generated_samples()) / 2;
  smp->inspect = [smp](uint64_t frame) {
    if(!kog_inspection_enabled()) return;
    KogVoice data[8]; KogVoices voices{data, 8};
    kog_inspect_snes(smp->dsp.spc_dsp.m.regs, voices);
    kog_inspection_publish(double(frame) / rate, data, voices.count);
  };
  emulator.set_fade(static_cast<long>(length), static_cast<long>(fade));
  Output output {file};
  output.bytes("KOGSFM1\0", 8);
  output.number(1, 4); output.number(rate, 4); output.number(2, 4);
  output.number(frames, 8); output.number(main_frames, 8);
  for(auto value : {info.system, info.song, info.game, info.author, info.copyright, info.date}) output.text(value);
  // Publish stream properties before the potentially long seek.
  if(std::fflush(file)) throw std::runtime_error("SFM stream closed");
  std::array<int16_t, 8192> samples {};
  std::array<uint8_t, 16384> pcm {};
  // Keep the same render blocks when seeking: GME's silence lookahead and
  // fade envelope depend on play() boundaries, and skip() bypasses them.
  // Discard the prefix only after rendering it so a seek is an exact suffix.
  for(uint64_t position = 0; position < frames;) {
    if(kog_embedded_stream_cancelled(file)) throw std::runtime_error("SFM playback cancelled");
    const auto count = std::min<uint64_t>(4096, frames - position);
    check(emulator.play(static_cast<long>(count * 2), samples.data()));
    const auto discard = std::min(count, start > position ? start - position : 0);
    for(size_t i = discard * 2; i < count * 2; ++i) {
      pcm[i * 2] = static_cast<uint16_t>(samples[i]) & 255;
      pcm[i * 2 + 1] = static_cast<uint16_t>(samples[i]) >> 8;
    }
    if(discard < count) output.bytes(pcm.data() + discard * 4, static_cast<size_t>((count - discard) * 4));
    position += count;
  }
}
}
extern "C" int kog_sfm_embedded_run(const char* path, uint64_t start_frame,
    uint32_t length_ms, uint32_t fade_ms, intptr_t descriptor, char* error, size_t capacity) noexcept {
  File output(kog_embedded_stream_open(descriptor));
  try {
    if(!output) throw std::runtime_error("Cannot open SFM stream");
    render(path, start_frame, length_ms, fade_ms, output.get());
    if(std::fflush(output.get())) throw std::runtime_error("SFM stream closed");
    return 0;
  } catch(const std::exception& failure) {
    if(capacity) std::snprintf(error, capacity, "%s", failure.what());
    return -1;
  }
}
#ifdef KOG_SFM_TEST_DRIVER
int main(int argc, char** argv) {
  if(argc != 3) return 2;
  try { render(argv[1], std::stoull(argv[2]), 150000, 8000, stdout); return 0; }
  catch(const std::exception& error) { std::fprintf(stderr, "%s\n", error.what()); return 1; }
}
#endif
