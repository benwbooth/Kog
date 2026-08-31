#include "hively_bridge.h"

#include "hvl_replay.h"

#include <limits.h>
#include <stdlib.h>

void hvl_play_irq(struct hvl_tune *tune);

struct KogHively {
    struct hvl_tune *tune;
    uint32_t subsong;
    uint64_t song_frames;
    uint64_t fade_frames;
    uint64_t total_frames;
    uint64_t frames_read;
    int16_t *pcm;
    size_t pcm_frames;
    size_t pcm_position;
    size_t frame_block;
};

void kog_hively_init(void) {
    hvl_InitReplayer();
}

static void set_error(int *error, int value) {
    if (error) {
        *error = value;
    }
}

static void reset_decoder(KogHively *decoder) {
    hvl_InitSubsong(decoder->tune, decoder->subsong);
    decoder->frames_read = 0;
    decoder->pcm_frames = 0;
    decoder->pcm_position = 0;
}

KogHively *kog_hively_open(
    const uint8_t *data,
    size_t data_size,
    uint32_t sample_rate,
    uint32_t subsong,
    uint32_t loop_count,
    uint64_t fade_frames,
    int *error
) {
    KogHively *decoder;
    struct hvl_tune *tune;
    uint64_t safety;
    uint32_t loops = 0;

    set_error(error, KOG_HIVELY_OK);
    if (!data || data_size < 16 || data_size > UINT_MAX || sample_rate == 0 || loop_count == 0) {
        set_error(error, KOG_HIVELY_INVALID_FILE);
        return NULL;
    }

    tune = hvl_ParseTune(data, (uint32_t)data_size, sample_rate, 2);
    if (!tune) {
        set_error(error, KOG_HIVELY_INVALID_FILE);
        return NULL;
    }
    if (subsong > tune->ht_SubsongNr || !hvl_InitSubsong(tune, subsong)) {
        hvl_FreeTune(tune);
        set_error(error, KOG_HIVELY_INVALID_SUBSONG);
        return NULL;
    }

    safety = UINT64_C(2) * 60 * 60 * 50 * tune->ht_SpeedMultiplier;
    while (loops < loop_count && safety > 0) {
        hvl_play_irq(tune);
        --safety;
        if (tune->ht_SongEndReached) {
            tune->ht_SongEndReached = 0;
            ++loops;
        }
    }
    if (loops < loop_count) {
        hvl_FreeTune(tune);
        set_error(error, KOG_HIVELY_DURATION_LIMIT);
        return NULL;
    }

    decoder = calloc(1, sizeof(*decoder));
    if (!decoder) {
        hvl_FreeTune(tune);
        set_error(error, KOG_HIVELY_OUT_OF_MEMORY);
        return NULL;
    }
    decoder->tune = tune;
    decoder->subsong = subsong;
    decoder->song_frames =
        ((uint64_t)tune->ht_PlayingTime * sample_rate) /
        ((uint64_t)tune->ht_SpeedMultiplier * 50);
    decoder->fade_frames = fade_frames;
    decoder->total_frames = decoder->song_frames + fade_frames;
    if (decoder->total_frames < decoder->song_frames) {
        kog_hively_free(decoder);
        set_error(error, KOG_HIVELY_DURATION_LIMIT);
        return NULL;
    }
    decoder->frame_block =
        (sample_rate / 50 / tune->ht_SpeedMultiplier) * tune->ht_SpeedMultiplier;
    if (decoder->frame_block == 0 || decoder->frame_block > SIZE_MAX / (sizeof(int16_t) * 2)) {
        kog_hively_free(decoder);
        set_error(error, KOG_HIVELY_DURATION_LIMIT);
        return NULL;
    }
    decoder->pcm = malloc(decoder->frame_block * sizeof(int16_t) * 2);
    if (!decoder->pcm) {
        kog_hively_free(decoder);
        set_error(error, KOG_HIVELY_OUT_OF_MEMORY);
        return NULL;
    }

    reset_decoder(decoder);
    return decoder;
}

void kog_hively_free(KogHively *decoder) {
    if (!decoder) {
        return;
    }
    if (decoder->tune) {
        hvl_FreeTune(decoder->tune);
    }
    free(decoder->pcm);
    free(decoder);
}

uint32_t kog_hively_subsong_count(const KogHively *decoder) {
    return decoder ? (uint32_t)decoder->tune->ht_SubsongNr + 1 : 0;
}

uint32_t kog_hively_selected_subsong(const KogHively *decoder) {
    return decoder ? decoder->subsong : 0;
}

const char *kog_hively_title(const KogHively *decoder) {
    return decoder ? decoder->tune->ht_Name : NULL;
}

uint64_t kog_hively_total_frames(const KogHively *decoder) {
    return decoder ? decoder->total_frames : 0;
}

static void decode_frame(KogHively *decoder) {
    hvl_DecodeFrame(
        decoder->tune,
        (int8 *)decoder->pcm,
        (int8 *)decoder->pcm + sizeof(int16_t),
        sizeof(int16_t) * 2
    );
    decoder->pcm_frames = decoder->frame_block;
    decoder->pcm_position = 0;
}

size_t kog_hively_render(KogHively *decoder, float *output, size_t frames) {
    size_t written = 0;

    if (!decoder || !output) {
        return 0;
    }
    while (written < frames && decoder->frames_read < decoder->total_frames) {
        size_t available;
        size_t requested;
        size_t index;

        if (decoder->pcm_position == decoder->pcm_frames) {
            decode_frame(decoder);
        }
        available = decoder->pcm_frames - decoder->pcm_position;
        requested = frames - written;
        if (requested > available) {
            requested = available;
        }
        if ((uint64_t)requested > decoder->total_frames - decoder->frames_read) {
            requested = (size_t)(decoder->total_frames - decoder->frames_read);
        }

        for (index = 0; index < requested; ++index) {
            uint64_t position = decoder->frames_read + index;
            float scale = 1.0f;
            size_t input = (decoder->pcm_position + index) * 2;
            size_t out = (written + index) * 2;
            if (position >= decoder->song_frames && decoder->fade_frames > 0) {
                scale = (float)(decoder->total_frames - position) /
                        (float)decoder->fade_frames;
            }
            output[out] = (float)decoder->pcm[input] * (scale / 32768.0f);
            output[out + 1] = (float)decoder->pcm[input + 1] * (scale / 32768.0f);
        }

        decoder->pcm_position += requested;
        decoder->frames_read += requested;
        written += requested;
    }
    return written;
}

uint64_t kog_hively_seek(KogHively *decoder, uint64_t frame) {
    uint64_t full_blocks;
    uint64_t block;
    size_t remainder;
    uint32_t irq;

    if (!decoder) {
        return 0;
    }
    if (frame > decoder->total_frames) {
        frame = decoder->total_frames;
    }
    reset_decoder(decoder);

    full_blocks = frame / decoder->frame_block;
    remainder = (size_t)(frame % decoder->frame_block);
    for (block = 0; block < full_blocks; ++block) {
        for (irq = 0; irq < decoder->tune->ht_SpeedMultiplier; ++irq) {
            hvl_play_irq(decoder->tune);
        }
    }
    decoder->frames_read = full_blocks * decoder->frame_block;
    if (remainder > 0) {
        decode_frame(decoder);
        decoder->pcm_position = remainder;
        decoder->frames_read += remainder;
    }
    return decoder->frames_read;
}
