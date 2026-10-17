/* Fixed-profile half-rate adapter. Source library: LGPL-2.1-or-later.
 * Preserve the split-channel float32, HQ, 96000-input-frame cadence of
 * Python-SoXR 1.0.0; no external callbacks or global mutable state. */
#include "soxr.h"
#include <stddef.h>
#include <stdio.h>

int cw_soxr_half(const float *input, size_t count, float *output, size_t capacity,
                 size_t *written, char *message, size_t message_capacity) {
    soxr_error_t error = NULL;
    soxr_t state = NULL;
    size_t position = 0;
    if (!written) return 1;
    *written = 0;
    if (!input || !output || capacity < count / 2 + 1) {
        error = "invalid half-rate buffer";
    } else {
        soxr_io_spec_t io = soxr_io_spec(SOXR_FLOAT32_S, SOXR_FLOAT32_S);
        soxr_quality_spec_t quality = soxr_quality_spec(SOXR_HQ, 0);
        state = soxr_create(2, 1, 1, &error, &io, &quality, NULL);
        if (!error && !state) error = "soxr_create returned null";
    }
    for (size_t start = 0; !error && start < count; start += 96000) {
        size_t length = count - start;
        if (length > 96000) length = 96000;
        const float *in_channels[] = {input + start};
        float *out_channels[] = {output + position};
        size_t consumed = 0, done = 0;
        error = soxr_process(state, in_channels, length, &consumed,
                             out_channels, capacity - position, &done);
        if (!error && (consumed != length || done > capacity - position))
            error = "incomplete half-rate resampling";
        if (!error) position += done;
    }
    if (!error) {
        float *out_channels[] = {output + position};
        size_t done = 0;
        error = soxr_process(state, NULL, 0, NULL,
                             out_channels, capacity - position, &done);
        if (!error && done > capacity - position) error = "invalid half-rate output count";
        if (!error) position += done;
    }
    if (error && message && message_capacity) snprintf(message, message_capacity, "%s", error);
    if (state) soxr_delete(state);
    *written = position;
    return error ? 1 : 0;
}
