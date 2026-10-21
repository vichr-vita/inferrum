package inferrum

import (
	"errors"
	"fmt"
	"math"
	"runtime"
	"strings"
	"sync"
	"testing"

	"google.golang.org/protobuf/proto"
	"inferrum.local/ffi/bonusv1"
)

func engineForTest(t *testing.T) *BonusEngine {
	t.Helper()
	engine, err := NewBonusEngine()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = engine.Close() })
	return engine
}

// This is E01 from the existing Rust fixtures, with a caller-owned employee ID.
func sampleInput() *bonusv1.BonusInput {
	return &bonusv1.BonusInput{
		EmployeeId: proto.String("E01"), Active: proto.Bool(true),
		TenureMonths: proto.Uint32(48), PerformanceScore: proto.Uint32(94),
		SalaryCents: proto.Uint64(8_000_000), Role: bonusv1.Role_ROLE_SPECIALIST.Enum(),
		Department:       bonusv1.Department_DEPARTMENT_ENGINEERING.Enum(),
		TrainingComplete: proto.Bool(true), DisciplinaryHold: proto.Bool(false),
		CompanyTargetMet: proto.Bool(true), EmployeeBudgetCents: proto.Uint64(1_200_000),
		ManagerApproved: proto.Bool(true), FinanceApproved: proto.Bool(true), CriticalSkill: proto.Bool(true),
	}
}

func TestEvaluateBonus(t *testing.T) {
	engine := engineForTest(t)
	input := sampleInput()
	// Exercise owned Rust strings, including embedded NUL and UTF-8, rather than C strings.
	id := "employee\x00" + strings.Repeat("ž", 1024)
	input.EmployeeId = proto.String(id)
	response, err := engine.Evaluate(&bonusv1.BonusRequest{Inputs: input, IncludeTrace: true})
	if err != nil {
		t.Fatal(err)
	}
	if response.PolicyVersion != "bonus.v1" || len(response.GetResult().Trace) != 26 {
		t.Fatalf("unexpected policy or trace: %v", response)
	}
	facts := response.GetResult().Facts
	if facts.FinalBonusCents == nil || *facts.FinalBonusCents != 1_049_000 {
		t.Fatalf("unexpected bonus: %v", facts)
	}
	if facts.GetTotalCompensationCents() != 9_049_000 || facts.GetPaymentReadyCents() != 1_049_000 {
		t.Fatalf("unexpected amounts: %v", facts)
	}
	if facts.GetPaymentReference() != fmt.Sprintf("BONUS-%s-1049000", id) || facts.ReviewRequired != nil {
		t.Fatalf("unexpected reference or review: %v", facts)
	}
	// Response fields must survive after the Rust response buffer and engine are freed.
	if err := engine.Close(); err != nil {
		t.Fatal(err)
	}
	runtime.GC()
	if facts.GetPaymentReference() != fmt.Sprintf("BONUS-%s-1049000", id) {
		t.Fatal("response retained Rust-owned memory")
	}
}

func TestPresenceAndIndependentEvaluations(t *testing.T) {
	engine := engineForTest(t)
	input := sampleInput()
	input.ManagerApproved = nil
	missing, err := engine.Evaluate(&bonusv1.BonusRequest{Inputs: input, IncludeTrace: true})
	if err != nil {
		t.Fatal(err)
	}
	if missing.GetResult().Facts.FinalBonusCents != nil {
		t.Fatal("absent approval should skip the final bonus rule")
	}
	var skipped bool
	for _, step := range missing.GetResult().Trace {
		if step.Rule == "approvals_satisfied" {
			skipped = step.Status == bonusv1.RuleStatus_RULE_STATUS_SKIPPED &&
				len(step.MissingInputs) == 1 && step.MissingInputs[0] == "manager_approved"
		}
	}
	if !skipped {
		t.Fatal("missing input was not reported in the trace")
	}

	input.ManagerApproved = proto.Bool(false)
	denied, err := engine.Evaluate(&bonusv1.BonusRequest{Inputs: input, IncludeTrace: true})
	if err != nil {
		t.Fatal(err)
	}
	facts := denied.GetResult().Facts
	if facts.FinalBonusCents == nil || *facts.FinalBonusCents != 0 || facts.GetReviewRequired() != "approval_pending" || facts.PaymentReadyCents != nil {
		t.Fatalf("false approval lost its presence: %v", facts)
	}
	var noMatch bool
	for _, step := range denied.GetResult().Trace {
		if step.Rule == "payment_ready" {
			noMatch = step.Status == bonusv1.RuleStatus_RULE_STATUS_NO_MATCH
		}
	}
	if !noMatch {
		t.Fatal("unmatched rule was not reported")
	}

	input.EmployeeBudgetCents = proto.Uint64(0)
	input.ManagerApproved = proto.Bool(true)
	zero, err := engine.Evaluate(&bonusv1.BonusRequest{Inputs: input})
	if err != nil || zero.GetResult().Facts.CappedBonusCents == nil || zero.GetResult().Facts.GetCappedBonusCents() != 0 {
		t.Fatalf("zero budget lost its presence: %v, %v", zero, err)
	}
	empty, err := engine.Evaluate(&bonusv1.BonusRequest{})
	if err != nil {
		t.Fatal(err)
	}
	if !proto.Equal(empty.GetResult().Facts, &bonusv1.BonusFacts{}) || len(empty.GetResult().Trace) != 0 {
		t.Fatal("evaluation retained facts or returned an unrequested trace")
	}
}

func TestStructuredErrors(t *testing.T) {
	engine := engineForTest(t)
	for _, tc := range []struct {
		name        string
		change      func(*bonusv1.BonusInput)
		code        bonusv1.ErrorCode
		field, rule string
	}{
		{"score", func(i *bonusv1.BonusInput) { i.PerformanceScore = proto.Uint32(101) }, bonusv1.ErrorCode_ERROR_CODE_INVALID_INPUT, "performance_score", ""},
		{"score narrowing", func(i *bonusv1.BonusInput) { i.PerformanceScore = proto.Uint32(256) }, bonusv1.ErrorCode_ERROR_CODE_INVALID_INPUT, "performance_score", ""},
		{"tenure narrowing", func(i *bonusv1.BonusInput) { i.TenureMonths = proto.Uint32(65536) }, bonusv1.ErrorCode_ERROR_CODE_INVALID_INPUT, "tenure_months", ""},
		{"role", func(i *bonusv1.BonusInput) { i.Role = bonusv1.Role(999).Enum() }, bonusv1.ErrorCode_ERROR_CODE_INVALID_INPUT, "role", ""},
		{"department", func(i *bonusv1.BonusInput) { i.Department = bonusv1.Department_DEPARTMENT_UNSPECIFIED.Enum() }, bonusv1.ErrorCode_ERROR_CODE_INVALID_INPUT, "department", ""},
		{"overflow", func(i *bonusv1.BonusInput) { i.SalaryCents = proto.Uint64(math.MaxUint64) }, bonusv1.ErrorCode_ERROR_CODE_RULE_FAILED, "", "total_compensation"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			input := sampleInput()
			tc.change(input)
			response, err := engine.Evaluate(&bonusv1.BonusRequest{Inputs: input})
			var detail *EvaluationError
			if response != nil || !errors.As(err, &detail) {
				t.Fatalf("expected structured error without partial result: %v, %v", response, err)
			}
			if detail.Details.Code != tc.code || detail.Details.GetField() != tc.field || detail.Details.GetRule() != tc.rule || detail.PolicyVersion != "bonus.v1" {
				t.Fatalf("unexpected error details: %v", detail)
			}
		})
	}
	_, err := engine.evaluateBytes([]byte{0xff})
	var detail *EvaluationError
	if !errors.As(err, &detail) || detail.Details.Code != bonusv1.ErrorCode_ERROR_CODE_DECODE {
		t.Fatalf("expected decode error: %v", err)
	}
	if _, err := engine.Evaluate(nil); err == nil {
		t.Fatal("nil request was accepted")
	}
}

func TestConcurrentCallsAndClose(t *testing.T) {
	engine := engineForTest(t)
	var calls sync.WaitGroup
	for range 8 {
		calls.Add(1)
		go func() {
			defer calls.Done()
			for range 20 {
				response, err := engine.Evaluate(&bonusv1.BonusRequest{Inputs: sampleInput()})
				if err != nil || response.GetResult().Facts.GetFinalBonusCents() != 1_049_000 {
					t.Errorf("concurrent evaluation: %v, %v", response, err)
					return
				}
			}
		}()
	}
	calls.Wait()

	start := make(chan struct{})
	for range 8 {
		calls.Add(1)
		go func() {
			defer calls.Done()
			<-start
			_, err := engine.Evaluate(&bonusv1.BonusRequest{})
			if err != nil && !errors.Is(err, ErrClosed) {
				t.Errorf("evaluation racing with Close: %v", err)
			}
		}()
	}
	close(start)
	_ = engine.Close()
	calls.Wait()
	_ = engine.Close()
	if _, err := engine.Evaluate(&bonusv1.BonusRequest{}); !errors.Is(err, ErrClosed) {
		t.Fatalf("evaluation after Close: %v", err)
	}
	var zero BonusEngine
	_ = zero.Close()
	if _, err := zero.Evaluate(&bonusv1.BonusRequest{}); !errors.Is(err, ErrClosed) {
		t.Fatalf("zero-value engine: %v", err)
	}
}
