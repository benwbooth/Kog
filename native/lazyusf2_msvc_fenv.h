#ifndef KOG_LAZYUSF2_MSVC_FENV_H
#define KOG_LAZYUSF2_MSVC_FENV_H

/*
 * LazyUSF2 uses __control87_2 to update both x87 and SSE rounding state, but
 * Microsoft does not provide that routine on x64 or ARM64. _controlfp_s is
 * the supported equivalent for changing the rounding-control bits there.
 */
#include <float.h>

static __inline int kog_control87_2(
    unsigned int new_control,
    unsigned int mask,
    unsigned int *x87_control,
    unsigned int *sse2_control
) {
    unsigned int current_control = 0;
    int result = _controlfp_s(&current_control, new_control, mask);
    if (x87_control != 0) {
        *x87_control = current_control;
    }
    if (sse2_control != 0) {
        *sse2_control = current_control;
    }
    return result == 0;
}

#define __control87_2 kog_control87_2

#endif
