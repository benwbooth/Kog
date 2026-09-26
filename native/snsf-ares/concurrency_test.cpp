// SPDX-License-Identifier: GPL-3.0-or-later
#include "snsf.hpp"
#include <thread>
#include <future>
#include <atomic>
#include <chrono>
#include <stdexcept>
#include <unistd.h>
#include <signal.h>
#include <pthread.h>
#include <cstring>
static thread_local bool cancel = false;
extern "C" bool kog_decoder_cancelled() { return cancel; }
static std::vector<uint8_t> run(const char* path, uint64_t start = 0) {
    FILE* file = tmpfile();
    if(!file) throw std::runtime_error("tmpfile failed");
    char error[512]{};
    const int result = kog_snsf_embedded_run(path, start, 1000, 0, dup(fileno(file)), error, sizeof(error));
    if(result) throw std::runtime_error(error);
    fseek(file, 0, SEEK_END); auto size = ftell(file); rewind(file);
    std::vector<uint8_t> data(size);
    if(fread(data.data(), 1, data.size(), file) != data.size()) throw std::runtime_error("read failed");
    fclose(file); return data;
}
int main(int argc, char** argv) {
    if(argc != 2) return 2;
    signal(SIGPIPE, SIG_IGN);
    try {
        const auto expected = run(argv[1]);
        // Match Rust's usual 2 MiB decoder worker stack, including native TLS.
        struct SmallWorker { const char* path; std::vector<uint8_t> output; std::exception_ptr error; } small{argv[1]};
        pthread_attr_t attr; pthread_attr_init(&attr);
        pthread_attr_setstacksize(&attr, 2 * 1024 * 1024);
        pthread_t worker;
        const int created = pthread_create(&worker, &attr, [](void* opaque) -> void* {
            auto& state = *static_cast<SmallWorker*>(opaque);
            try { state.output = run(state.path); } catch(...) { state.error = std::current_exception(); }
            return nullptr;
        }, &small);
        pthread_attr_destroy(&attr);
        if(created) throw std::runtime_error("could not start 2 MiB worker");
        pthread_join(worker, nullptr);
        if(small.error) std::rethrow_exception(small.error);
        if(small.output != expected) throw std::runtime_error("2 MiB worker output differs");
        auto first = std::async(std::launch::async, [&] { return run(argv[1]); });
        auto second = std::async(std::launch::async, [&] { return run(argv[1]); });
        if(first.get() != expected || second.get() != expected) throw std::runtime_error("concurrent output differs");
        // Stall one reader: another live renderer must still complete.
        int pipefd[2]; if(pipe(pipefd)) throw std::runtime_error("pipe failed");
        auto stalled = std::async(std::launch::async, [&] {
            char error[512]{};
            return kog_snsf_embedded_run(argv[1], 0, 1000, 0, pipefd[1], error, sizeof(error));
        });
        auto independent = std::async(std::launch::async, [&] { return run(argv[1]); });
        const bool timely = independent.wait_for(std::chrono::seconds(5)) == std::future_status::ready;
        close(pipefd[0]);
        auto actual = independent.get();
        if(!timely) throw std::runtime_error("renderer blocked behind another stream (timeout)");
        if(actual != expected) throw std::runtime_error("independent stream PCM differs");
        if(stalled.get() == 0) throw std::runtime_error("closed output stream was ignored");
        cancel = true;
        char error[512]{}; FILE* sink = tmpfile();
        const int result = kog_snsf_embedded_run(argv[1], 16000, 1000, 0, dup(fileno(sink)), error, sizeof(error));
        fclose(sink); cancel = false;
        if(result == 0 || !strstr(error,"cancel")) throw std::runtime_error("seek cancellation was ignored");
        if(run(argv[1]) != expected) throw std::runtime_error("state leaked after cancellation");
        puts("SNSF deterministic concurrent playback, stalled-reader isolation and seek cancellation passed");
        return 0;
    } catch(const std::exception& e) { fprintf(stderr,"%s\n",e.what()); return 1; }
}
