/* Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
 * Linux test interposer: abort any attempt to allocate/protect executable pages.
 * Dynamic-loader initial mappings happen before symbol interposition; Play!'s
 * JIT allocations happen afterward and are caught (verified with the old helper).
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <sys/mman.h>
#include <unistd.h>
static void reject_exec(void)
{
    static const char message[] = "Kog test: executable-memory request rejected\n";
    (void)write(STDERR_FILENO, message, sizeof(message) - 1);
    _exit(97);
}
void* mmap(void* address, size_t length, int protection, int flags, int fd, off_t offset)
{
    if(protection & PROT_EXEC) reject_exec();
    void* (*real_mmap)(void*, size_t, int, int, int, off_t) = dlsym(RTLD_NEXT, "mmap");
    return real_mmap(address, length, protection, flags, fd, offset);
}
int mprotect(void* address, size_t length, int protection)
{
    if(protection & PROT_EXEC) reject_exec();
    int (*real_mprotect)(void*, size_t, int) = dlsym(RTLD_NEXT, "mprotect");
    return real_mprotect(address, length, protection);
}
