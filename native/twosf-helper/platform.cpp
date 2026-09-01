/*
 * Headless platform services for Kog's melonDS 2SF helper.
 * Copyright (C) 2026 Kog contributors.
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

#include "Platform.h"

#include <chrono>
#include <condition_variable>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <mutex>
#include <string>
#include <thread>

#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace melonDS::Platform
{
namespace
{
bool hasMode(FileMode mode, FileMode flag)
{
    return (static_cast<unsigned>(mode) & static_cast<unsigned>(flag)) != 0U;
}

std::string fileMode(FileMode mode, bool exists)
{
    char access = 'r';
    if(hasMode(mode, FileMode::Append))
        access = 'a';
    else if(hasMode(mode, FileMode::Write) &&
            !(hasMode(mode, FileMode::Preserve) && exists) &&
            !hasMode(mode, FileMode::NoCreate))
        access = 'w';

    std::string result(1, access);
    if(hasMode(mode, FileMode::Read) && hasMode(mode, FileMode::Write)) result += '+';
    if(!hasMode(mode, FileMode::Text)) result += 'b';
    return result;
}

const auto processStart = std::chrono::steady_clock::now();
}

struct FileHandle
{
    std::FILE* file = nullptr;
};

struct Thread
{
    explicit Thread(std::function<void()> function) : thread(std::move(function)) {}
    std::thread thread;
};

struct Semaphore
{
    std::mutex mutex;
    std::condition_variable condition;
    unsigned int count = 0;
};

struct Mutex
{
    std::mutex mutex;
};

struct AACDecoder {};

struct DynamicLibrary
{
#ifdef _WIN32
    HMODULE handle = nullptr;
#else
    void* handle = nullptr;
#endif
};

void SignalStop(StopReason reason, void* userdata)
{
    (void)reason;
    (void)userdata;
}

std::string GetLocalFilePath(const std::string& filename)
{
    return filename;
}

FileHandle* OpenFile(const std::string& path, FileMode mode)
{
    if(!hasMode(mode, FileMode::Read) && !hasMode(mode, FileMode::Write) &&
       !hasMode(mode, FileMode::Append))
        return nullptr;

    const bool exists = std::filesystem::exists(std::filesystem::u8path(path));
    if(hasMode(mode, FileMode::NoCreate) && !exists) return nullptr;
    const std::string modeString = fileMode(mode, exists);
#ifdef _WIN32
    const std::wstring wideMode(modeString.begin(), modeString.end());
    std::FILE* file = _wfopen(std::filesystem::u8path(path).c_str(), wideMode.c_str());
#else
    std::FILE* file = std::fopen(path.c_str(), modeString.c_str());
#endif
    if(file == nullptr) return nullptr;
    return new FileHandle {file};
}

FileHandle* OpenLocalFile(const std::string& path, FileMode mode)
{
    return OpenFile(GetLocalFilePath(path), mode);
}

bool FileExists(const std::string& name)
{
    std::error_code error;
    return std::filesystem::is_regular_file(std::filesystem::u8path(name), error);
}

bool LocalFileExists(const std::string& name)
{
    return FileExists(GetLocalFilePath(name));
}

bool CheckFileWritable(const std::string& filepath)
{
    FileHandle* file = OpenFile(filepath, FileMode::Append);
    if(file == nullptr) return false;
    return CloseFile(file);
}

bool CheckLocalFileWritable(const std::string& filepath)
{
    return CheckFileWritable(GetLocalFilePath(filepath));
}

bool CloseFile(FileHandle* file)
{
    if(file == nullptr) return false;
    const int result = std::fclose(file->file);
    delete file;
    return result == 0;
}

bool IsEndOfFile(FileHandle* file)
{
    return file == nullptr || std::feof(file->file) != 0;
}

bool FileReadLine(char* str, int count, FileHandle* file)
{
    return file != nullptr && std::fgets(str, count, file->file) != nullptr;
}

u64 FilePosition(FileHandle* file)
{
    if(file == nullptr) return 0;
#ifdef _WIN32
    const auto position = _ftelli64(file->file);
#else
    const auto position = ftello(file->file);
#endif
    return position < 0 ? 0 : static_cast<u64>(position);
}

bool FileSeek(FileHandle* file, s64 offset, FileSeekOrigin origin)
{
    if(file == nullptr) return false;
    int nativeOrigin = SEEK_SET;
    if(origin == FileSeekOrigin::Current) nativeOrigin = SEEK_CUR;
    else if(origin == FileSeekOrigin::End) nativeOrigin = SEEK_END;
#ifdef _WIN32
    return _fseeki64(file->file, offset, nativeOrigin) == 0;
#else
    return fseeko(file->file, static_cast<off_t>(offset), nativeOrigin) == 0;
#endif
}

void FileRewind(FileHandle* file)
{
    if(file != nullptr) std::rewind(file->file);
}

u64 FileRead(void* data, u64 size, u64 count, FileHandle* file)
{
    if(file == nullptr) return 0;
    return std::fread(data, static_cast<size_t>(size), static_cast<size_t>(count), file->file);
}

bool FileFlush(FileHandle* file)
{
    return file != nullptr && std::fflush(file->file) == 0;
}

u64 FileWrite(const void* data, u64 size, u64 count, FileHandle* file)
{
    if(file == nullptr) return 0;
    return std::fwrite(data, static_cast<size_t>(size), static_cast<size_t>(count), file->file);
}

u64 FileWriteFormatted(FileHandle* file, const char* format, ...)
{
    if(file == nullptr) return 0;
    va_list arguments;
    va_start(arguments, format);
    const int result = std::vfprintf(file->file, format, arguments);
    va_end(arguments);
    return result < 0 ? 0 : static_cast<u64>(result);
}

u64 FileLength(FileHandle* file)
{
    if(file == nullptr) return 0;
    const u64 original = FilePosition(file);
    if(!FileSeek(file, 0, FileSeekOrigin::End)) return 0;
    const u64 length = FilePosition(file);
    if(!FileSeek(file, static_cast<s64>(original), FileSeekOrigin::Start)) return 0;
    return length;
}

void Log(LogLevel level, const char* format, ...)
{
    if(level < LogLevel::Warn) return;
    std::fputs(level == LogLevel::Error ? "melonDS error: " : "melonDS warning: ", stderr);
    va_list arguments;
    va_start(arguments, format);
    std::vfprintf(stderr, format, arguments);
    va_end(arguments);
}

Thread* Thread_Create(std::function<void()> function)
{
    try { return new Thread(std::move(function)); }
    catch(...) { return nullptr; }
}

void Thread_Free(Thread* thread)
{
    if(thread == nullptr) return;
    if(thread->thread.joinable()) thread->thread.detach();
    delete thread;
}

void Thread_Wait(Thread* thread)
{
    if(thread != nullptr && thread->thread.joinable()) thread->thread.join();
}

Semaphore* Semaphore_Create()
{
    try { return new Semaphore; }
    catch(...) { return nullptr; }
}

void Semaphore_Free(Semaphore* semaphore)
{
    delete semaphore;
}

void Semaphore_Reset(Semaphore* semaphore)
{
    if(semaphore == nullptr) return;
    const std::lock_guard<std::mutex> lock(semaphore->mutex);
    semaphore->count = 0;
}

void Semaphore_Wait(Semaphore* semaphore)
{
    if(semaphore == nullptr) return;
    std::unique_lock<std::mutex> lock(semaphore->mutex);
    semaphore->condition.wait(lock, [semaphore] { return semaphore->count != 0; });
    --semaphore->count;
}

bool Semaphore_TryWait(Semaphore* semaphore, int timeoutMilliseconds)
{
    if(semaphore == nullptr) return false;
    std::unique_lock<std::mutex> lock(semaphore->mutex);
    const bool ready = timeoutMilliseconds <= 0
        ? semaphore->count != 0
        : semaphore->condition.wait_for(lock, std::chrono::milliseconds(timeoutMilliseconds),
                                        [semaphore] { return semaphore->count != 0; });
    if(ready) --semaphore->count;
    return ready;
}

void Semaphore_Post(Semaphore* semaphore, int count)
{
    if(semaphore == nullptr || count <= 0) return;
    {
        const std::lock_guard<std::mutex> lock(semaphore->mutex);
        semaphore->count += static_cast<unsigned int>(count);
    }
    semaphore->condition.notify_all();
}

Mutex* Mutex_Create()
{
    try { return new Mutex; }
    catch(...) { return nullptr; }
}

void Mutex_Free(Mutex* mutex) { delete mutex; }
void Mutex_Lock(Mutex* mutex) { if(mutex != nullptr) mutex->mutex.lock(); }
void Mutex_Unlock(Mutex* mutex) { if(mutex != nullptr) mutex->mutex.unlock(); }
bool Mutex_TryLock(Mutex* mutex) { return mutex != nullptr && mutex->mutex.try_lock(); }

void Sleep(u64 microseconds)
{
    std::this_thread::sleep_for(std::chrono::microseconds(microseconds));
}

u64 GetMSCount()
{
    return static_cast<u64>(std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - processStart).count());
}

u64 GetUSCount()
{
    return static_cast<u64>(std::chrono::duration_cast<std::chrono::microseconds>(
        std::chrono::steady_clock::now() - processStart).count());
}

void WriteNDSSave(const u8*, u32, u32, u32, void*) {}
void WriteGBASave(const u8*, u32, u32, u32, void*) {}
void WriteFirmware(const Firmware&, u32, u32, void*) {}
void WriteDateTime(int, int, int, int, int, int, void*) {}

void MP_Begin(void*) {}
void MP_End(void*) {}
int MP_SendPacket(u8*, int, u64, void*) { return 0; }
int MP_RecvPacket(u8*, u64*, void*) { return 0; }
int MP_SendCmd(u8*, int, u64, void*) { return 0; }
int MP_SendReply(u8*, int, u64, u16, void*) { return 0; }
int MP_SendAck(u8*, int, u64, void*) { return 0; }
int MP_RecvHostPacket(u8*, u64*, void*) { return 0; }
u16 MP_RecvReplies(u8*, u64, u16, void*) { return 0; }
int Net_SendPacket(u8*, int, void*) { return 0; }
int Net_RecvPacket(u8*, void*) { return 0; }

void Camera_Start(int, void*) {}
void Camera_Stop(int, void*) {}
void Camera_CaptureFrame(int, u32* frame, int width, int height, bool, void*)
{
    if(frame != nullptr && width > 0 && height > 0)
        std::memset(frame, 0, static_cast<size_t>(width) * static_cast<size_t>(height) * sizeof(u32));
}

void Mic_Start(void*) {}
void Mic_Stop(void*) {}
int Mic_ReadInput(s16* data, int maximumLength, void*)
{
    if(data != nullptr && maximumLength > 0)
        std::memset(data, 0, static_cast<size_t>(maximumLength) * sizeof(s16));
    return maximumLength > 0 ? maximumLength : 0;
}

AACDecoder* AAC_Init() { return nullptr; }
void AAC_DeInit(AACDecoder* decoder) { delete decoder; }
bool AAC_Configure(AACDecoder*, int, int) { return false; }
bool AAC_DecodeFrame(AACDecoder*, const void*, int, void*, int) { return false; }

bool Addon_KeyDown(KeyType, void*) { return false; }
void Addon_RumbleStart(u32, void*) {}
void Addon_RumbleStop(void*) {}
float Addon_MotionQuery(MotionQueryType, void*) { return 0.0F; }

DynamicLibrary* DynamicLibrary_Load(const char* library)
{
    if(library == nullptr) return nullptr;
    auto result = new DynamicLibrary;
#ifdef _WIN32
    result->handle = LoadLibraryA(library);
#else
    result->handle = dlopen(library, RTLD_NOW | RTLD_LOCAL);
#endif
    if(result->handle == nullptr)
    {
        delete result;
        return nullptr;
    }
    return result;
}

void DynamicLibrary_Unload(DynamicLibrary* library)
{
    if(library == nullptr) return;
#ifdef _WIN32
    FreeLibrary(library->handle);
#else
    dlclose(library->handle);
#endif
    delete library;
}

void* DynamicLibrary_LoadFunction(DynamicLibrary* library, const char* name)
{
    if(library == nullptr || name == nullptr) return nullptr;
#ifdef _WIN32
    return reinterpret_cast<void*>(GetProcAddress(library->handle, name));
#else
    return dlsym(library->handle, name);
#endif
}
}
