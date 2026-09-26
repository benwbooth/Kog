// SPDX-License-Identifier: GPL-3.0-or-later
#include "snsf.hpp"
#include "../embedded_stream.h"
#include <exception>
#include <cstdlib>
extern "C" int kog_snsf_embedded_run(const char* path, uint64_t start, uint32_t length,
    uint32_t fade, intptr_t descriptor, char* error, size_t capacity) noexcept {
    FILE* output = kog_embedded_stream_open(descriptor);
    if(!output) { if(capacity) std::snprintf(error, capacity, "Cannot open SNSF PCM stream"); return -1; }
    int result = 0;
    try { auto image = kog::snsf::load(path); kog::snsf::render(image, output, start, length, fade); }
    catch(const std::exception& e) { if(capacity) std::snprintf(error, capacity, "%s", e.what()); result = -1; }
    catch(...) { if(capacity) std::snprintf(error, capacity, "Unknown SNSF renderer failure"); result = -1; }
    if(std::fclose(output) && result == 0) { if(capacity) std::snprintf(error, capacity, "SNSF PCM stream closed"); result = -1; }
    return result;
}
