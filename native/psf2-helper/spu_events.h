#pragma once
#include <array>
#include <cstdint>
#include <unordered_map>

// Each PSF renderer owns its emulation thread. Count actual key-on writes
// without adding state to Play!'s SPU, changing save states, or changing audio.
inline auto& kog_spu_note_events()
{
    static thread_local std::unordered_map<const void*, std::array<uint64_t, 24>> events;
    return events;
}
inline void kog_spu_reset_events(const void* spu) { kog_spu_note_events().erase(spu); }
inline void kog_spu_note_on(const void* spu, unsigned voice) { ++kog_spu_note_events()[spu][voice]; }
inline uint64_t kog_spu_note_on_count(const void* spu, unsigned voice)
{
    const auto& events = kog_spu_note_events();
    const auto found = events.find(spu);
    return found == events.end() ? 0 : found->second[voice];
}
