// SPDX-License-Identifier: GPL-3.0-or-later
#include "snsf.hpp"
#include <exception>
#include <stdexcept>
int main(int argc, char** argv) {
    if(argc != 2) return 2;
    try {
        auto image = kog::snsf::load(argv[1]);
        if(image.rom.size() != 32768 || image.rom[259] != 0x42 ||
           image.sram[4] != 0x42 || image.sram[5] != 0xaa || image.sram[6] != 0xbb ||
           image.tags.at("title") != "Root")
            throw std::runtime_error("SNSF library-relative ROM/SRAM/tag precedence mismatch");
        return 0;
    } catch(const std::exception& e) { fprintf(stderr,"%s\n",e.what()); return 1; }
}
