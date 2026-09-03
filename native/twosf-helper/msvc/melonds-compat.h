/*
 * Force-included into melonDS's core target on MSVC. The pin leans on GCC
 * builtins and attributes that MSVC lacks; map them onto intrinsics. These
 * are bit-scan helpers and optimization hints, not behavior.
 */

#pragma once

#ifdef _MSC_VER

#define __attribute(x)
#define __builtin_unreachable() ((void)0)

#include <intrin.h>

static __inline int __builtin_ctz(unsigned int value)
{
    unsigned long index;
    _BitScanForward(&index, value);
    return (int)index;
}

static __inline int __builtin_ctzll(unsigned long long value)
{
    unsigned long index;
    _BitScanForward64(&index, value);
    return (int)index;
}

#endif
