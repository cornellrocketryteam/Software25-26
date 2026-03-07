#include "StepperMotor.h"
#include <cmath>

StepperMotor::StepperMotor(uint8_t DIR, uint8_t STEP, uint8_t EN,
                           int stepsPerRev, int microsteps)
    : DIR_(DIR), STEP_(STEP), stepsPerRev_(stepsPerRev),
      microsteps_(microsteps), currentAngle_(0.0),
      motor_(AccelStepper::DRIVER, STEP, DIR) {
  motor_.setMaxSpeed(1000);
  motor_.setAcceleration(500);

  if (EN != 255) {
    motor_.setEnablePin(EN);
    // Default active low enable for most stepper drivers (e.g. TMC, A4988)
    motor_.setPinsInverted(false, false, true);
  }
}

void StepperMotor::begin() {
  pinMode(DIR_, OUTPUT);
  pinMode(STEP_, OUTPUT);
  motor_.enableOutputs();
}

long StepperMotor::angleToSteps(double angle) const {
  double stepsPerDegree = ((double)stepsPerRev_ * (double)microsteps_) / 360.0;
  return (long)round(angle * stepsPerDegree);
}

void StepperMotor::moveAngleTo(double targetAngle) {
  // Constrain incoming target to [0, 360)
  targetAngle = fmod(targetAngle, 360.0);
  if (targetAngle < 0) {
    targetAngle += 360.0;
  }

  // Calculate shortest path difference
  double difference = targetAngle - currentAngle_;
  if (difference > 180.0) {
    difference -= 360.0;
  } else if (difference < -180.0) {
    difference += 360.0;
  }

  // Move by the difference
  long stepsToMove = angleToSteps(difference);
  motor_.move(stepsToMove); // move() is relative, moveTo() is absolute

  currentAngle_ = targetAngle;
}

bool StepperMotor::isRunning() { return motor_.distanceToGo() != 0; }

long StepperMotor::currentPosition() { return motor_.currentPosition(); }

double StepperMotor::currentAngle() { return currentAngle_; }

void StepperMotor::update() { motor_.run(); }

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