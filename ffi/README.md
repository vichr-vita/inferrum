# Call Inferrum from Go

The `inferrum-ffi` crate builds the fictional bonus policy into a Rust static
library. Go supplies typed Protobuf inputs through cgo and receives typed derived
facts. Everything runs inside the Go process. There is no server or gRPC runtime.

This first contract exposes the existing bonus policy. It does not load arbitrary
rulesets or execute Go callbacks. Other policies need their own compiled Rust
rules, Protobuf messages, conversion code, and constructor.

## Build and run

You need Rust 1.85 or later, Go 1.23 or later, a native C compiler, and `protoc`
3.15 or later on `PATH`. The supplied Go linker configuration supports native
Linux and macOS builds. The library and Go application must target the same
architecture. Windows and cross-compilation are not configured.

From the repository root:

```sh
cargo build --locked -p inferrum-ffi --release
cd ffi/go
go run -a ./cmd/bonus
```

The Go example evaluates E01 from the Rust fixtures and prints:

```text
policy=bonus.v1 bonus_cents=1049000 reference=BONUS-E01-1049000 rules=26
```

The static library is `target/release/libinferrum_ffi.a`. The Go wrapper links it
directly, so rebuild the library after changing Rust code or the schema. Go's
build cache does not track changes to native libraries. Use `go run -a`,
`go build -a`, or `go test -a` after rebuilding the Rust archive. Keep the default
Cargo target directory when using the supplied linker configuration.
Ordinary core-crate builds do not build the FFI crate or require `protoc`.

## Use it in your Go application

The Go module is local and unpublished. Run these commands in your application's
module directory, substituting the absolute checkout path:

```sh
go mod edit -replace=inferrum.local/ffi=/absolute/path/to/inferrum/ffi/go
go get inferrum.local/ffi
```

Import `inferrum.local/ffi` and `inferrum.local/ffi/bonusv1`. Create the engine once
and reuse it for independent cases:

```go
engine, err := inferrum.NewBonusEngine()
if err != nil {
    return err
}
defer engine.Close()

response, err := engine.Evaluate(&bonusv1.BonusRequest{
    Inputs: fetchedInputs,
    IncludeTrace: true,
})
if err != nil {
    return err
}
facts := response.GetResult().Facts
if facts.FinalBonusCents != nil {
    fmt.Println(*facts.FinalBonusCents)
}
```

Here `fetchedInputs` is a `*bonusv1.BonusInput`. See
[`go/cmd/bonus/main.go`](go/cmd/bonus/main.go) for a complete example. Enable cgo
with `CGO_ENABLED=1` if your environment disables it.

Evaluations can share an engine concurrently. `Close` waits for active calls and
is safe to repeat. Calls after close return `inferrum.ErrClosed`. Do not copy an
engine value. Each owner must call `Close`; there is no automatic finalizer.
Evaluation is synchronous. A Go context cannot interrupt an in-progress call.

## Facts and errors

[`proto/bonus.proto`](proto/bonus.proto) is the data contract. All 14 input fields
and 26 possible derived facts track presence. Use `proto.Bool(false)` or
`proto.Uint64(0)` to supply a present false or zero value. Leave a field nil to
omit it. Missing inputs skip dependent rules while independent rules continue.
An empty request succeeds with no derived facts. A missing output is not zero.

The adapter rejects scores outside 0..100, tenure outside 0..65535, and explicitly
supplied unspecified or unknown enum values. It does not substitute defaults.
Unknown Protobuf fields are ignored. No input facts are returned or retained
between evaluations.

The response includes `policy_version`, currently `bonus.v1`, independently of
the `inferrum.bonus.v1` schema package. Change the policy identifier when rule
behavior changes. Evolve the schema with Protobuf compatibility rules, retaining
field numbers and reserving deleted fields.

Errors have a code, message, and optional field or rule identifier. The response
contains either a result or an error, never a partial result. The Go wrapper
turns Rust errors into `*inferrum.EvaluationError`, which you can inspect with
`errors.As`. Its `Details` field contains the generated Protobuf error;
`PolicyVersion` identifies the evaluated policy.

Set `include_trace` to return all rule outcomes in evaluation order. Trace entries
distinguish produced facts, no match, and skipped rules with missing inputs.

## C ownership contract

[`include/inferrum.h`](include/inferrum.h) declares four operations:

| Operation | Ownership |
|---|---|
| `inferrum_bonus_new` | Returns an engine, or owned `EvaluationError` bytes |
| `inferrum_bonus_evaluate` | Borrows request bytes for the call and returns owned `BonusResponse` bytes |
| `inferrum_buffer_free` | Releases an unchanged returned buffer exactly once |
| `inferrum_engine_free` | Releases a handle once, after its evaluations finish |

The Go wrapper decodes returned bytes into Go-owned fields before freeing the
Rust allocation. Rust never retains Go pointers. C callers must provide valid
pointers and lengths, keep handles alive during evaluation, and synchronize
destruction against calls. Null engine and invalid request arguments produce
structured errors. Null releases are allowed. Freed pointers cannot be reused.

Rust callback errors preserve the failing rule identifier. Unwinding panics are
caught at the C interface and returned as internal errors. Panic payloads are
left allocated to prevent a second panic from their destructor. Abort-on-panic
builds and allocation failure can terminate the process. Keep the default
unwinding panic strategy for this bridge.

## Generate and verify

Rust messages are generated during Cargo builds. Go messages are checked in.
After editing the schema, regenerate Go bindings from the repository root:

```sh
sh ffi/generate-go.sh
```

The script installs `protoc-gen-go` v1.36.6 into `target/protobuf-bin`. It does
not change globally installed tools. Rebuild the Rust library before Go tests.

```sh
cargo fmt --all --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo build --locked -p inferrum-ffi --release
cd ffi/go
go vet ./...
GOEXPERIMENT=cgocheck2 go test -race -a ./...
```

Rust tests exercise the encoded C interface against all 14 existing scenarios,
presence, validation, malformed bytes, callback errors, and panic containment.
Go tests exercise cgo calls, owned strings and response lifetimes, false and zero
values, missing inputs, arithmetic errors, concurrent evaluation, and close.
CI runs Rust checks on 1.85 and stable, verifies generated Go bindings, and runs
the Go checks on Linux and macOS.
