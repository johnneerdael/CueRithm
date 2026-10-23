// Where Mixxx's MP3 source puts the first click, on Linux and Windows.
//
// Mixxx decodes MP3 with libmad there (SoundSourceMp3). It scans the file and skips the
// FIRST frame unconditionally, whether or not that frame is the Xing/Info tag (see the
// comment in src/sources/soundsourcemp3.cpp), so the track's frame 0 is the first sample of
// the second frame, and everything is decoded with mad_frame_decode and mad_synth_frame.
// This does the same and prints the frame whose left channel first exceeds a threshold.
//
// Build and run (needs libmad, for example in the Debian image of this folder):
//   gcc -O2 mad_probe.c -o mad_probe -lmad -lm && ./mad_probe click.mp3 [threshold]
#include <math.h>
#include <mad.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s file.mp3 [threshold]\n", argv[0]);
        return 2;
    }
    const double threshold = argc > 2 ? atof(argv[2]) : 0.3;
    FILE *file = fopen(argv[1], "rb");
    if (!file) {
        perror("open");
        return 1;
    }
    fseek(file, 0, SEEK_END);
    long size = ftell(file);
    fseek(file, 0, SEEK_SET);
    unsigned char *data = malloc((size_t)size + MAD_BUFFER_GUARD);
    if (fread(data, 1, (size_t)size, file) != (size_t)size) {
        fprintf(stderr, "short read\n");
        return 1;
    }
    for (int i = 0; i < MAD_BUFFER_GUARD; i++) {
        data[size + i] = 0;
    }
    fclose(file);

    struct mad_stream stream;
    struct mad_header header;
    struct mad_frame frame;
    struct mad_synth synth;
    mad_stream_init(&stream);
    mad_header_init(&header);
    mad_frame_init(&frame);
    mad_synth_init(&synth);
    mad_stream_buffer(&stream, data, (unsigned long)size + MAD_BUFFER_GUARD);

    int skipped = 0;
    long index = 0;
    for (;;) {
        if (!skipped) {
            // The first valid frame is dropped, as Mixxx does.
            if (mad_header_decode(&header, &stream)) {
                if (MAD_RECOVERABLE(stream.error)) {
                    continue;
                }
                break;
            }
            skipped = 1;
            continue;
        }
        if (mad_frame_decode(&frame, &stream)) {
            if (MAD_RECOVERABLE(stream.error)) {
                continue;
            }
            break;
        }
        mad_synth_frame(&synth, &frame);
        for (unsigned i = 0; i < synth.pcm.length; i++) {
            if (fabs(mad_f_todouble(synth.pcm.samples[0][i])) > threshold) {
                printf("mixxx_frame=%ld rate=%u\n", index, frame.header.samplerate);
                return 0;
            }
            index++;
        }
    }
    fprintf(stderr, "no click found\n");
    return 1;
}
