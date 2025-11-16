#include "StepperMotor.h"
#include <cmath>

StepperMotor::StepperMotor(uint8_t DIR, uint8_t STEP, int stepsPerRev, int microsteps)
    : DIR_(DIR), STEP_(STEP), stepsPerRev_(stepsPerRev), microsteps_(microsteps), currentAngle_(0.0), motor_(AccelStepper::DRIVER, STEP, DIR)
    {
        motor_.setMaxSpeed(1000);
        motor_.setAcceleration(500);
    }

int StepperMotor::angleToSteps(double angle) const {
    double stepsPerDegree = ((double)stepsPerRev_ * (double)microsteps_) / 360.0;
    return (long)round(angle * stepsPerDegree);
}

void StepperMotor::moveAngleTo(double targetAngle) {
    if (targetAngle < 0) {
        targetAngle += 360.0;
    } else if (targetAngle > 360.0) {
        targetAngle -= 360.0;
    }
    double difference = targetAngle - currentAngle_;

    if (difference > 180.0) {
        difference -= 360.0;
    }
    if (difference < -180.0) {
        difference += 360.0;
    }
    int targetSteps = motor_.currentPosition() + angleToSteps(difference);
    motor_.moveTo(targetSteps);
    currentAngle_ = targetAngle;
}

void StepperMotor::update() {
    motor_.run();
}

void StepperMotor::setMaxSpeed(float stepsPerSec) {
    motor_.setMaxSpeed(stepsPerSec);
}

void StepperMotor::setAcceleration(float stepsPerSec2) {
    motor_.setAcceleration(stepsPerSec2);
}

void StepperMotor::reset() {
    motor_.setCurrentPosition(0.0);
    currentAngle_ = 0.0;
}