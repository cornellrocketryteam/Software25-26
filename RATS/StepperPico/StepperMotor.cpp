#include "StepperMotor.h"
#include <cmath>

StepperMotor::StepperMotor(uint8_t DIR, uint8_t STEP, int stepsPerRev, int microsteps)
    : DIR_(DIR), STEP_(STEP), stepsPerRev_(stepsPerRev), microsteps_(microsteps), currentAngle_(0.0), motor_(AccelStepper::DRIVER, STEP, DIR)
    {
        motor_.setMaxSpeed(1000);
        motor_.setAcceleration(500);
    }

long StepperMotor::angleToSteps(double angle) const {
    double stepsPerDegree = (stepsPerRev_ * microsteps_) / 360.0;
    return lround(angle * stepsPerDegree);
}

void StepperMotor::moveAngleTo(double targetAngle) {
    targetAngle = fmod(targetAngle, 360.0);
    if (targetAngle < 0) targetAngle += 360.0;

    double difference = targetAngle - currentAngle_;

    if (difference > 180.0) difference -= 360.0;
    if (difference < -180.0) difference += 360.0;

    long targetSteps =
        motor_.currentPosition() + angleToSteps(difference);

    motor_.moveTo(targetSteps);
}

bool StepperMotor::isRunning() {
    return motor_.isRunning();
}

void StepperMotor::run() {
    motor_.run();

    // Update angle only when done moving
    if (motor_.distanceToGo() == 0) {
        long steps = motor_.currentPosition();
        double degreesPerStep =
            360.0 / (stepsPerRev_ * microsteps_);
        currentAngle_ = fmod(steps * degreesPerStep, 360.0);
        if (currentAngle_ < 0) currentAngle_ += 360.0;
    }
}

void StepperMotor::setMaxSpeed(float stepsPerSec) {
    motor_.setMaxSpeed(stepsPerSec);
}

void StepperMotor::setAcceleration(float stepsPerSec2) {
    motor_.setAcceleration(stepsPerSec2);
}

void StepperMotor::reset() {
    motor_.setCurrentPosition(0);
    currentAngle_ = 0.0;
}

void StepperMotor::home() {
    motor_.moveTo(0);
}

double StepperMotor::getCurrentAngle() {
    return currentAngle_;
}