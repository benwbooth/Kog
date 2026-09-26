// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once
#include <cstdint>
#include <cstddef>
#include <cstdio>
#include <map>
#include <string>
#include <vector>
namespace kog::snsf {
struct Image {
    std::vector<uint8_t> rom;
    std::vector<uint8_t> sram = std::vector<uint8_t>(131072, 255);
    std::vector<bool> sramWritten = std::vector<bool>(131072);
    std::map<std::string, std::string> tags;
};
Image load(const char* path);
void render(Image&, FILE*, uint64_t start, uint32_t length, uint32_t fade);
}
extern "C" int kog_snsf_embedded_run(const char*, uint64_t, uint32_t, uint32_t,
    intptr_t, char*, size_t) noexcept;
