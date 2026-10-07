// SPDX-License-Identifier: GPL-3.0-or-later
#include "snsf.hpp"
#include "../inspection_standalone.h"
#include <exception>
#include <cstdlib>
extern "C" bool kog_decoder_cancelled() { return false; }
int main(int argc, char** argv) {
    if(argc != 5) { std::fprintf(stderr,"usage: kog-snsf-ares PATH START_FRAME LENGTH_MS FADE_MS\n"); return 2; }
    try {
        auto image = kog::snsf::load(argv[1]);
        kog::snsf::render(image, stdout, std::stoull(argv[2]), std::stoul(argv[3]), std::stoul(argv[4]));
        return 0;
    } catch(const std::exception& e) { std::fprintf(stderr,"%s\n",e.what()); return 1; }
}
