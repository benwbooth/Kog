/* Original synthetic tones only. Exercise the exact bridge used by Rust. */
#include "vgmstream_bridge.h"
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <speex/speex.h>
#include <libatrac9.h>

int kog_celt_probe_0061(void);
int kog_celt_probe_0110(void);

static int probe_speex(void) {
    void *encoder = speex_encoder_init(&speex_nb_mode);
    void *decoder = speex_decoder_init(&speex_nb_mode);
    SpeexBits bits;
    speex_bits_init(&bits);
    double energy = 0;
    if (!encoder || !decoder) return 1;
    for (int frame = 0; frame < 20; ++frame) {
        short input[160], output[160];
        char packet[512];
        for (int i = 0; i < 160; ++i) input[i] = (short)(12000 * sin((frame * 160 + i) * 440 * 6.28318530718 / 8000));
        speex_bits_reset(&bits);
        if (speex_encode_int(encoder, input, &bits) < 0) return 1;
        int size = speex_bits_write(&bits, packet, sizeof(packet));
        if (size <= 0) return 1;
        speex_bits_read_from(&bits, packet, size);
        if (speex_decode_int(decoder, &bits, output) < 0) return 1;
        for (int i = 0; i < 160; ++i) energy += (double)output[i] * output[i];
    }
    speex_bits_destroy(&bits);
    speex_encoder_destroy(encoder);
    speex_decoder_destroy(decoder);
    return energy < 1000000;
}

int main(int argc, char **argv) {
    const char *names[] = {"tone.msf", "tone.logg", "tone.m4a"};
    const char *codecs[] = {"MPEG", "Vorbis", "AAC"};
    if (argc != 2) return 2;
    if (kog_celt_probe_0061() || kog_celt_probe_0110() || probe_speex()) {
        fprintf(stderr, "CELT/Speex encode/decode regression\n"); return 1;
    }
    unsigned char config[] = {0xfe, 0x60, 0x0f, 0xe0};
    void *atrac9 = Atrac9GetHandle();
    Atrac9CodecInfo info;
    if (!atrac9 || Atrac9InitDecoder(atrac9, config) || Atrac9GetCodecInfo(atrac9, &info)
        || info.channels != 1 || info.samplingRate != 44100) return 1;
    Atrac9ReleaseHandle(atrac9);
    printf("CELT 0.6.1/0.11 and Speex PCM round trips; ATRAC9 initialization: OK\n");
    for (unsigned i = 0; i < 3; ++i) {
        char path[4096];
        int error = 0;
        snprintf(path, sizeof(path), "%s/%s", argv[1], names[i]);
        KogVgmstream *decoder = kog_vgmstream_open(path, -1, 1.0, 0.0, &error);
        if (!decoder) { fprintf(stderr, "Cannot open %s: %d\n", path, error); return 1; }
        const char *codec = kog_vgmstream_codec(decoder);
        printf("%s: %s\n", names[i], codec);
        if (!strstr(codec, codecs[i])) { fprintf(stderr, "Unexpected codec\n"); return 1; }
        if (kog_vgmstream_channels(decoder) != 1 || kog_vgmstream_total_frames(decoder) < 16000) return 1;
        for (int pass = 0; pass < 2; ++pass) {
            float pcm[4096];
            if (pass && kog_vgmstream_seek(decoder, 4410) != 4410) return 1;
            int64_t frames = kog_vgmstream_render(decoder, pcm, 4096);
            double energy = 0;
            if (frames <= 0 || frames > 4096) return 1;
            for (int64_t n = 0; n < frames; ++n) {
                if (!isfinite(pcm[n])) return 1;
                energy += pcm[n] * pcm[n];
            }
            if (energy < 1) { fprintf(stderr, "Silent output: %s\n", names[i]); return 1; }
        }
        kog_vgmstream_free(decoder);
    }
    return 0;
}
