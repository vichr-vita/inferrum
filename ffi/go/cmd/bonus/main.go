// Command bonus evaluates the existing E01 fixture through the Rust library.
package main

import (
	"fmt"
	"log"

	"google.golang.org/protobuf/proto"
	inferrum "inferrum.local/ffi"
	"inferrum.local/ffi/bonusv1"
)

func main() {
	if err := run(); err != nil {
		log.Fatal(err)
	}
}

func run() error {
	engine, err := inferrum.NewBonusEngine()
	if err != nil {
		return err
	}
	defer engine.Close()
	response, err := engine.Evaluate(&bonusv1.BonusRequest{
		Inputs: &bonusv1.BonusInput{
			EmployeeId: proto.String("E01"), Active: proto.Bool(true),
			TenureMonths: proto.Uint32(48), PerformanceScore: proto.Uint32(94),
			SalaryCents: proto.Uint64(8_000_000), Role: bonusv1.Role_ROLE_SPECIALIST.Enum(),
			Department:       bonusv1.Department_DEPARTMENT_ENGINEERING.Enum(),
			TrainingComplete: proto.Bool(true), DisciplinaryHold: proto.Bool(false),
			CompanyTargetMet: proto.Bool(true), EmployeeBudgetCents: proto.Uint64(1_200_000),
			ManagerApproved: proto.Bool(true), FinanceApproved: proto.Bool(true), CriticalSkill: proto.Bool(true),
		},
		IncludeTrace: true,
	})
	if err != nil {
		return err
	}
	facts := response.GetResult().Facts
	fmt.Printf("policy=%s bonus_cents=%d reference=%s rules=%d\n",
		response.PolicyVersion, facts.GetFinalBonusCents(), facts.GetPaymentReference(), len(response.GetResult().Trace))
	return nil
}
