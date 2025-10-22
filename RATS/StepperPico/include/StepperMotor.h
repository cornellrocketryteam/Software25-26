#pragma once
#include "pico/stdlib.h"
#include <AccelStepper.h>
#include <cmath>

class StepperMotor {
    public:
    StepperMotor(uint8_t DIR, uint8_t STEP, int stepsPerRev = 200, int microsteps = 16);
    
    void moveAngleTo(double targetAngle);
    void update();
    void reset();

    private:
        uint DIR_, STEP_;
        int stepsPerRev_, microsteps_;
        double currentAngle_;
        AccelStepper motor_;
        int angleToSteps(double angle) const;
}
