// Reports where Mixxx's frame timeline puts the first loud sample of a file on macOS.
//
// Mixxx decodes MP3, AAC, M4A and MP4 with Apple CoreAudio on macOS
// (SoundSourceCoreAudio, priority "Higher"). It opens the file with ExtAudioFile,
// asks the converter for its priming ("leading") frames, and declares the stream's
// frame index range to start at that number: `IndexRange::forward(leadingFrames,
// totalFrames)`. A sample that ExtAudioFile returns at output position `p` is
// therefore Mixxx frame `leadingFrames + p`.
//
// This program makes the same calls and prints that number for the first sample
// whose left channel exceeds a threshold, so a click track gives the Mixxx frame of
// its first click. Build and run:
//
//   clang -framework AudioToolbox -framework CoreFoundation coreaudio_probe.c -o probe
//   ./probe file.mp3
//
// Output (one line): leading=<frames> trailing=<frames> total=<frames> onset=<output
// position> mixxx_frame=<leading + onset> table_priming=<n> table_remainder=<n>
// table_valid=<n> (the last three are -1 when the file has no packet table).
#include <AudioToolbox/AudioToolbox.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s file [threshold]\n", argv[0]);
        return 2;
    }
    const float threshold = argc > 2 ? (float)atof(argv[2]) : 0.3f;
    CFURLRef url = CFURLCreateFromFileSystemRepresentation(
            NULL, (const UInt8*)argv[1], (CFIndex)strlen(argv[1]), false);
    ExtAudioFileRef file = NULL;
    OSStatus err = ExtAudioFileOpenURL(url, &file);
    CFRelease(url);
    if (err != noErr) {
        fprintf(stderr, "open failed: %d\n", (int)err);
        return 1;
    }
    AudioStreamBasicDescription input;
    UInt32 size = sizeof(input);
    ExtAudioFileGetProperty(file, kExtAudioFileProperty_FileDataFormat, &size, &input);
    // Mixxx's client format: 32-bit float, packed and interleaved, at most two channels
    // here (its own limit is higher; a stereo click track is enough for the probe).
    UInt32 channels = input.mChannelsPerFrame < 2 ? input.mChannelsPerFrame : 2;
    AudioStreamBasicDescription output;
    memset(&output, 0, sizeof(output));
    output.mSampleRate = input.mSampleRate;
    output.mFormatID = kAudioFormatLinearPCM;
    output.mFormatFlags = kAudioFormatFlagsNativeFloatPacked;
    output.mChannelsPerFrame = channels;
    output.mBitsPerChannel = 32;
    output.mFramesPerPacket = 1;
    output.mBytesPerFrame = channels * 4;
    output.mBytesPerPacket = output.mBytesPerFrame;
    err = ExtAudioFileSetProperty(
            file, kExtAudioFileProperty_ClientDataFormat, sizeof(output), &output);
    if (err != noErr) {
        fprintf(stderr, "client format failed: %d\n", (int)err);
        return 1;
    }
    SInt64 total = 0;
    size = sizeof(total);
    ExtAudioFileGetProperty(file, kExtAudioFileProperty_FileLengthFrames, &size, &total);
    AudioConverterRef converter = NULL;
    size = sizeof(converter);
    ExtAudioFileGetProperty(file, kExtAudioFileProperty_AudioConverter, &size, &converter);
    // The file's own gapless description (from a LAME tag or iTunSMPB), which
    // ExtAudioFile uses to decide how much leading decoder output to discard.
    SInt64 table_priming = -1;
    SInt64 table_remainder = -1;
    SInt64 table_valid = -1;
    {
        CFURLRef table_url = CFURLCreateFromFileSystemRepresentation(
                NULL, (const UInt8*)argv[1], (CFIndex)strlen(argv[1]), false);
        AudioFileID audio_file = NULL;
        if (AudioFileOpenURL(table_url, kAudioFileReadPermission, 0, &audio_file) == noErr) {
            AudioFilePacketTableInfo table;
            UInt32 table_size = sizeof(table);
            if (AudioFileGetProperty(audio_file, kAudioFilePropertyPacketTableInfo, &table_size, &table) == noErr) {
                table_priming = table.mPrimingFrames;
                table_remainder = table.mRemainderFrames;
                table_valid = table.mNumberValidFrames;
            }
            AudioFileClose(audio_file);
        }
        CFRelease(table_url);
    }
    AudioConverterPrimeInfo prime;
    memset(&prime, 0, sizeof(prime));
    size = sizeof(prime);
    err = AudioConverterGetProperty(converter, kAudioConverterPrimeInfo, &size, &prime);
    if (err != noErr) {
        // Mixxx treats this as "no leading frames".
        memset(&prime, 0, sizeof(prime));
    }
    const UInt32 block = 4096;
    float* buffer = (float*)malloc(block * output.mBytesPerFrame);
    SInt64 position = 0;
    SInt64 onset = -1;
    for (;;) {
        AudioBufferList list;
        list.mNumberBuffers = 1;
        list.mBuffers[0].mNumberChannels = channels;
        list.mBuffers[0].mDataByteSize = block * output.mBytesPerFrame;
        list.mBuffers[0].mData = buffer;
        UInt32 frames = block;
        err = ExtAudioFileRead(file, &frames, &list);
        if (err != noErr || frames == 0) {
            break;
        }
        for (UInt32 i = 0; i < frames && onset < 0; ++i) {
            if (fabsf(buffer[i * channels]) > threshold) {
                onset = position + i;
            }
        }
        position += frames;
        if (onset >= 0) {
            break;
        }
    }
    printf("leading=%d trailing=%d total=%lld onset=%lld mixxx_frame=%lld "
           "table_priming=%lld table_remainder=%lld table_valid=%lld\n",
            (int)prime.leadingFrames,
            (int)prime.trailingFrames,
            (long long)total,
            (long long)onset,
            (long long)(onset + prime.leadingFrames),
            (long long)table_priming,
            (long long)table_remainder,
            (long long)table_valid);
    free(buffer);
    ExtAudioFileDispose(file);
    return onset < 0 ? 1 : 0;
}
