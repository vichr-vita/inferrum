// Package inferrum evaluates the compiled Rust bonus policy through cgo.
package inferrum

//go:generate sh ../generate-go.sh

/*
#cgo CFLAGS: -I${SRCDIR}/../include
#cgo LDFLAGS: ${SRCDIR}/../../target/release/libinferrum_ffi.a
#cgo linux LDFLAGS: -ldl -lpthread -lm -lrt -lutil
#include "inferrum.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
	"sync"
	"unsafe"

	"google.golang.org/protobuf/proto"
	"inferrum.local/ffi/bonusv1"
)

// ErrClosed is returned after Close, including for a zero-value BonusEngine.
var ErrClosed = errors.New("inferrum: engine is closed")

// EvaluationError contains the structured error returned by Rust.
type EvaluationError struct {
	Details       *bonusv1.EvaluationError
	PolicyVersion string
}

func (e *EvaluationError) Error() string {
	return "inferrum: " + e.Details.GetMessage()
}

// BonusEngine owns a compiled Rust ruleset. Use NewBonusEngine and defer Close.
// Evaluations can run concurrently. Close waits for active evaluations to finish.
// A BonusEngine must not be copied after first use.
type BonusEngine struct {
	mu     sync.RWMutex
	handle *C.InferrumEngine
}

// NewBonusEngine constructs and validates the fictional bonus policy once.
func NewBonusEngine() (*BonusEngine, error) {
	created := C.inferrum_bonus_new()
	defer C.inferrum_buffer_free(created.error)
	if created.engine == nil {
		detail := new(bonusv1.EvaluationError)
		if err := decodeBuffer(created.error, detail); err != nil {
			return nil, err
		}
		return nil, &EvaluationError{Details: detail}
	}
	return &BonusEngine{handle: created.engine}, nil
}

// Evaluate supplies one independent case. Missing facts stay absent; false and
// zero remain present. Rust retains no request memory or facts between calls.
// Errors return no partial response. Evaluation is synchronous and cannot be
// interrupted by a Go context.
func (e *BonusEngine) Evaluate(request *bonusv1.BonusRequest) (*bonusv1.BonusResponse, error) {
	if request == nil {
		return nil, errors.New("inferrum: request must not be nil")
	}
	payload, err := proto.Marshal(request)
	if err != nil {
		return nil, fmt.Errorf("inferrum: encode request: %w", err)
	}
	return e.evaluateBytes(payload)
}

func (e *BonusEngine) evaluateBytes(payload []byte) (*bonusv1.BonusResponse, error) {
	e.mu.RLock()
	defer e.mu.RUnlock()
	if e.handle == nil {
		return nil, ErrClosed
	}
	// payload contains bytes only, and Rust borrows them for the duration of the call.
	buffer := C.inferrum_bonus_evaluate(
		e.handle, (*C.uint8_t)(unsafe.Pointer(unsafe.SliceData(payload))), C.size_t(len(payload)),
	)
	runtime.KeepAlive(payload)
	defer C.inferrum_buffer_free(buffer)
	response := new(bonusv1.BonusResponse)
	if err := decodeBuffer(buffer, response); err != nil {
		return nil, err
	}
	switch outcome := response.Outcome.(type) {
	case *bonusv1.BonusResponse_Result:
		if outcome.Result == nil || outcome.Result.Facts == nil {
			return nil, errors.New("inferrum: response has no derived facts message")
		}
		return response, nil
	case *bonusv1.BonusResponse_Error:
		return nil, &EvaluationError{Details: outcome.Error, PolicyVersion: response.PolicyVersion}
	default:
		return nil, errors.New("inferrum: response has no outcome")
	}
}

// decodeBuffer copies message data into Go-owned fields before the caller frees
// the Rust buffer. The temporary slice must never escape this function.
func decodeBuffer(buffer C.InferrumBuffer, message proto.Message) error {
	if buffer.data == nil || buffer.len == 0 || uint64(buffer.len) > uint64(^uint(0)>>1) {
		return errors.New("inferrum: invalid response buffer")
	}
	bytes := unsafe.Slice((*byte)(unsafe.Pointer(buffer.data)), int(buffer.len))
	if err := proto.Unmarshal(bytes, message); err != nil {
		return fmt.Errorf("inferrum: decode response: %w", err)
	}
	return nil
}

// Close releases the Rust handle. Repeated calls are safe. Evaluations started
// after Close returns fail with ErrClosed.
func (e *BonusEngine) Close() error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.handle != nil {
		C.inferrum_engine_free(e.handle)
		e.handle = nil
	}
	return nil
}
