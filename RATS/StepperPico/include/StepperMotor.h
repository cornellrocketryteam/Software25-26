#pragma once
#include <Arduino.h>
#include <AccelStepper.h>

class StepperMotor {
    public:
    StepperMotor(uint8_t DIR, uint8_t STEP, int stepsPerRev = 200, int microsteps = 16);
    
    void moveAngleTo(double targetAngle);
    void update();
    void setMaxSpeed(float stepsPerSec);
    void setAcceleration(float stepsPerSec2);
    void reset();

    private:
        uint8_t DIR_, STEP_;
        int stepsPerRev_, microsteps_;
        double currentAngle_;
        AccelStepper motor_;
        int angleToSteps(double angle) const;
}
