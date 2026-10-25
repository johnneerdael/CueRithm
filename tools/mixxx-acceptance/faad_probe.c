// Where Mixxx's FAAD-based M4A source puts the first click, on Linux builds that have it.
//
// SoundSourceM4A (src/sources/soundsourcem4a.cpp) opens the file with mp4v2, takes the first
// audio track and its elementary stream configuration, opens FAAD with float output and the
// stereo downmix, and decodes the sample blocks in order from the first one. It does not read
// the encoder's priming samples or the file's edit list, so frame 0 of the track is the first
// sample FAAD produces for the first block. This does the same and prints the frame whose left
// channel first exceeds a fraction of the file's own peak (FAAD's float scale differs between
// versions, so a fixed level could miss it).
//
// Build and run (needs libfaad and libmp4v2 with headers):
//   gcc -O2 faad_probe.c -o faad_probe -lfaad -lmp4v2 -lm && ./faad_probe click.m4a [fraction]
#include <faad.h>
#include <math.h>
#include <mp4v2/mp4v2.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s file.m4a [fraction of peak]\n", argv[0]);
        return 2;
    }
    const float fraction = argc > 2 ? (float)atof(argv[2]) : 0.3f;
    MP4FileHandle file = MP4Read(argv[1]);
    if (file == MP4_INVALID_FILE_HANDLE) {
        fprintf(stderr, "cannot open %s\n", argv[1]);
        return 1;
    }
    MP4TrackId track = MP4_INVALID_TRACK_ID;
    const uint32_t tracks = MP4GetNumberOfTracks(file, NULL, 0);
    for (uint32_t i = 0; i < tracks; i++) {
        MP4TrackId id = MP4FindTrackId(file, (uint16_t)i, NULL, 0);
        const char *type = MP4GetTrackType(file, id);
        if (type && !strcmp(type, MP4_AUDIO_TRACK_TYPE)) {
            track = id;
            break;
        }
    }
    if (track == MP4_INVALID_TRACK_ID) {
        fprintf(stderr, "no audio track\n");
        return 1;
    }
    uint8_t *config = NULL;
    uint32_t configSize = 0;
    MP4GetTrackESConfiguration(file, track, &config, &configSize);

    NeAACDecHandle decoder = NeAACDecOpen();
    NeAACDecConfigurationPtr settings = NeAACDecGetCurrentConfiguration(decoder);
    settings->outputFormat = FAAD_FMT_FLOAT;
    settings->downMatrix = 1;
    NeAACDecSetConfiguration(decoder, settings);
    unsigned long rate = 0;
    unsigned char channels = 0;
    if (NeAACDecInit2(decoder, config, configSize, &rate, &channels) < 0) {
        fprintf(stderr, "FAAD could not use the configuration\n");
        return 1;
    }

    const MP4SampleId blocks = MP4GetTrackNumberOfSamples(file, track);
    size_t capacity = 1 << 20, length = 0;
    float *left = malloc(capacity * sizeof(float));
    float peak = 0.0f;
    for (MP4SampleId id = 1; id <= blocks; id++) {
        uint8_t *input = NULL;
        uint32_t size = 0;
        if (!MP4ReadSample(file, track, id, &input, &size, NULL, NULL, NULL, NULL)) {
            break;
        }
        NeAACDecFrameInfo info;
        float *pcm = (float *)NeAACDecDecode(decoder, &info, input, size);
        free(input);
        if (info.error || !pcm) {
            continue;
        }
        const unsigned ch = info.channels ? info.channels : 1;
        for (unsigned long i = 0; i < info.samples / ch; i++) {
            if (length == capacity) {
                capacity *= 2;
                left = realloc(left, capacity * sizeof(float));
            }
            left[length] = pcm[i * ch];
            if (fabsf(left[length]) > peak) {
                peak = fabsf(left[length]);
            }
            length++;
        }
    }
    if (peak <= 0.0f) {
        fprintf(stderr, "no signal decoded\n");
        return 1;
    }
    for (size_t i = 0; i < length; i++) {
        if (fabsf(left[i]) > fraction * peak) {
            printf("mixxx_frame=%zu rate=%lu\n", i, rate);
            return 0;
        }
    }
    fprintf(stderr, "no click found\n");
    return 1;
}
