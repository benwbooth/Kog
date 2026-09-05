#pragma once
#include "rust/cxx.h"
void kogConfigureArchiveDecoder(rust::Fn<rust::String(rust::Slice<const uint8_t>)> decoder);
