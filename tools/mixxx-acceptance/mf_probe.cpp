// Where Mixxx's Media Foundation source puts the first click, on Windows.
//
// SoundSourceMediaFoundation (src/sources/soundsourcemediafoundation.cpp) opens the file with
// MFCreateSourceReaderFromURL, selects the first audio stream only and asks the source reader for
// 32-bit float PCM at the stream's own channel count and sample rate. It then reads samples in
// order from the start; the frame index of the first sample it returns is 0 (it does not use the
// sample time stamps while reading forward from the start). So Mixxx frame N is the Nth frame the
// reader delivers. This does the same and prints the frame whose left channel first exceeds
// a threshold, the same click detector as the other probes in this folder.
//
// Build and run (MinGW-w64 with the Media Foundation import libraries, or MSVC):
//   g++ -O2 -municode mf_probe.cpp -o mf_probe.exe -lmfplat -lmfreadwrite -lmfuuid -lole32
//   mf_probe.exe click.m4a [threshold]
#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>

#include <cmath>
#include <cstdio>
#include <cstdlib>

template <class T>
static void release(T** pointer) {
    if (*pointer) {
        (*pointer)->Release();
        *pointer = nullptr;
    }
}

static bool fail(const char* what, HRESULT hr) {
    fprintf(stderr, "%s failed: 0x%08lx\n", what, static_cast<unsigned long>(hr));
    return false;
}

int wmain(int argc, wchar_t** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: mf_probe file [threshold]\n");
        return 2;
    }
    const float threshold = argc > 2 ? static_cast<float>(_wtof(argv[2])) : 0.3f;
    constexpr DWORD kStream = MF_SOURCE_READER_FIRST_AUDIO_STREAM;
    constexpr UINT32 kBytesPerSample = sizeof(float);
    constexpr UINT32 kLeftoverSize = 4096; // as Mixxx

    if (FAILED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE))) {
        return 1;
    }
    if (FAILED(MFStartup(MF_VERSION))) {
        return 1;
    }
    IMFSourceReader* reader = nullptr;
    HRESULT hr = MFCreateSourceReaderFromURL(argv[1], nullptr, &reader);
    if (FAILED(hr)) {
        fail("MFCreateSourceReaderFromURL", hr);
        return 1;
    }
    reader->SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS, FALSE);
    reader->SetStreamSelection(kStream, TRUE);

    // configureMediaType: keep channels and rate, ask for float PCM
    IMFMediaType* type = nullptr;
    hr = reader->GetCurrentMediaType(kStream, &type);
    if (FAILED(hr)) {
        fail("GetCurrentMediaType", hr);
        return 1;
    }
    type->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Audio);
    type->SetGUID(MF_MT_SUBTYPE, MFAudioFormat_Float);
    type->SetUINT32(MF_MT_ALL_SAMPLES_INDEPENDENT, TRUE);
    type->SetUINT32(MF_MT_FIXED_SIZE_SAMPLES, TRUE);
    type->SetUINT32(MF_MT_AUDIO_BITS_PER_SAMPLE, kBytesPerSample * 8);
    type->SetUINT32(MF_MT_SAMPLE_SIZE, kLeftoverSize * kBytesPerSample);
    hr = reader->SetCurrentMediaType(kStream, nullptr, type);
    release(&type);
    if (FAILED(hr)) {
        fail("SetCurrentMediaType", hr);
        return 1;
    }
    reader->SetStreamSelection(kStream, TRUE);
    hr = reader->GetCurrentMediaType(kStream, &type);
    if (FAILED(hr)) {
        fail("GetCurrentMediaType (output)", hr);
        return 1;
    }
    UINT32 channels = 0, rate = 0;
    type->GetUINT32(MF_MT_AUDIO_NUM_CHANNELS, &channels);
    type->GetUINT32(MF_MT_AUDIO_SAMPLES_PER_SECOND, &rate);
    release(&type);
    if (channels == 0) {
        return 1;
    }

    long long frames = 0;      // frames delivered so far = Mixxx's frame index
    long long first_time = -1; // time stamp of the first sample, 100 ns units
    long long click = -1;
    for (;;) {
        DWORD flags = 0;
        LONGLONG stamp = 0;
        IMFSample* sample = nullptr;
        hr = reader->ReadSample(kStream, 0, nullptr, &flags, &stamp, &sample);
        if (FAILED(hr) || (flags & (MF_SOURCE_READERF_ERROR | MF_SOURCE_READERF_ENDOFSTREAM))) {
            release(&sample);
            break;
        }
        if (!sample) {
            continue;
        }
        if (first_time < 0) {
            first_time = stamp;
        }
        IMFMediaBuffer* buffer = nullptr;
        if (SUCCEEDED(sample->ConvertToContiguousBuffer(&buffer))) {
            BYTE* data = nullptr;
            DWORD length = 0;
            if (SUCCEEDED(buffer->Lock(&data, nullptr, &length))) {
                const float* samples = reinterpret_cast<const float*>(data);
                const long long count = length / kBytesPerSample / channels;
                for (long long i = 0; i < count && click < 0; i++) {
                    if (std::fabs(samples[i * channels]) > threshold) {
                        click = frames + i;
                    }
                }
                frames += count;
                buffer->Unlock();
            }
            release(&buffer);
        }
        release(&sample);
        if (click >= 0) {
            break;
        }
    }
    release(&reader);
    MFShutdown();
    CoUninitialize();
    if (click < 0) {
        fprintf(stderr, "no click found\n");
        return 1;
    }
    printf("mixxx_frame=%lld rate=%u channels=%u first_sample_time_100ns=%lld\n", click, rate,
            channels, first_time);
    return 0;
}
