#include "celt.h"
#include <math.h>
#include <stdio.h>

int KOG_CELT_PROBE(void) {
    int error = 0;
#if KOG_CELT_OLD
    CELTMode *mode = celt_mode_create(44100, 1, 512, &error);
    CELTEncoder *encoder = mode ? celt_encoder_create(mode) : NULL;
    CELTDecoder *decoder = mode ? celt_decoder_create(mode) : NULL;
#else
    CELTMode *mode = celt_mode_create(44100, 512, &error);
    CELTEncoder *encoder = mode ? celt_encoder_create_custom(mode, 1, &error) : NULL;
    CELTDecoder *decoder = mode ? celt_decoder_create_custom(mode, 1, &error) : NULL;
#endif
    if (error || !encoder || !decoder) return 1;
    double energy = 0;
    for (int frame = 0; frame < 20; ++frame) {
        short input[512], output[512];
        unsigned char packet[128];
        for (int i = 0; i < 512; ++i) input[i] = (short)(12000 * sin((frame * 512 + i) * 440 * 6.28318530718 / 44100));
#if KOG_CELT_OLD
        int size = celt_encode(encoder, input, NULL, packet, sizeof(packet));
        if (size <= 0 || celt_decode(decoder, packet, size, output) < 0) return 1;
#else
        int size = celt_encode(encoder, input, 512, packet, sizeof(packet));
        if (size <= 0 || celt_decode(decoder, packet, size, output, 512) < 0) return 1;
#endif
        for (int i = 0; i < 512; ++i) energy += (double)output[i] * output[i];
    }
    celt_encoder_destroy(encoder);
    celt_decoder_destroy(decoder);
    celt_mode_destroy(mode);
    return energy < 1000000;
}
