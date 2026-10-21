#ifndef INFERRUM_H
#define INFERRUM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct InferrumEngine InferrumEngine;

// Rust-owned bytes. Free exactly once with inferrum_buffer_free, never free().
typedef struct {
    uint8_t *data;
    size_t len;
} InferrumBuffer;

typedef struct {
    InferrumEngine *engine;
    // Encoded EvaluationError on failure; {NULL, 0} on success.
    InferrumBuffer error;
} InferrumCreateResult;

// Validates the compiled bonus policy once. The caller owns the returned handle.
InferrumCreateResult inferrum_bonus_new(void);

// Accepts BonusRequest bytes and returns BonusResponse bytes.
// Request memory is borrowed only until this call returns. NULL is valid only
// with request_len == 0. A NULL engine returns an INVALID_INPUT error.
// Calls may share a live handle concurrently; destroy must not overlap them.
InferrumBuffer inferrum_bonus_evaluate(
    const InferrumEngine *engine, const uint8_t *request, size_t request_len);

// Accepts {NULL, 0}. Otherwise pass an unchanged buffer from this library once.
void inferrum_buffer_free(InferrumBuffer buffer);

// Accepts NULL. Otherwise pass a live handle once, after all calls finish.
void inferrum_engine_free(InferrumEngine *engine);

#ifdef __cplusplus
}
#endif
#endif
