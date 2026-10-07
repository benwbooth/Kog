// Bind a library built with KOG_EMBEDDED to the standalone file-ring transport.
#pragma once
#define kog_inspection_enabled kog_file_inspection_enabled
#define kog_inspection_publish kog_file_inspection_publish
#include "inspection.h"
#undef kog_inspection_enabled
#undef kog_inspection_publish
extern "C" bool kog_inspection_enabled() { return kog_file_inspection_enabled(); }
extern "C" void kog_inspection_publish(double position, const KogVoice* voices, size_t count) {
    kog_file_inspection_publish(position, voices, count);
}
