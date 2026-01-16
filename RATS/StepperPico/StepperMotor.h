#pragma once
#include <Arduino.h>
#include <AccelStepper.h>

class StepperMotor {
    public:
    StepperMotor(uint8_t DIR, uint8_t STEP, int stepsPerRev = 200, int microsteps = 8);
    
    void moveAngleTo(double targetAngle);
    void run();
    void setMaxSpeed(float stepsPerSec);
    void setAcceleration(float stepsPerSec2);
    void reset();
    bool isRunning();
    void home();
    double getCurrentAngle();


    private:
        AccelStepper motor_;
        uint8_t DIR_, STEP_;
        int stepsPerRev_, microsteps_;
        double currentAngle_;
        long angleToSteps(double angle) const;
};
