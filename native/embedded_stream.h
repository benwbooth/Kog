/* SPDX-License-Identifier: GPL-3.0-or-later
 * Private PCM output stream for an in-process renderer. The caller transfers
 * ownership of the pipe/socket writer; no process-wide stdout is redirected. */
#ifndef KOG_EMBEDDED_STREAM_H
#define KOG_EMBEDDED_STREAM_H

#include <stdint.h>
#include <stdio.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
static FILE* kog_embedded_stream_open(intptr_t descriptor)
{
    const int fd = _open_osfhandle(descriptor, _O_WRONLY | _O_BINARY);
    if(fd < 0)
    {
        CloseHandle((HANDLE)descriptor);
        return NULL;
    }
    FILE* output = _fdopen(fd, "wb");
    if(!output) _close(fd);
    return output;
}
#else
#include <unistd.h>
static FILE* kog_embedded_stream_open(intptr_t descriptor)
{
    FILE* output = fdopen((int)descriptor, "wb");
    if(!output) close((int)descriptor);
    return output;
}
#endif

#endif
