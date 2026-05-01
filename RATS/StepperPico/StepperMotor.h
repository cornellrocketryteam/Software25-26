#pragma once
#include "AccelStepper-1.64.0/src/AccelStepper.h"
#include <Arduino.h>

class StepperMotor {
public:
  StepperMotor(uint8_t DIR, uint8_t STEP, uint8_t EN = 255,
               int stepsPerRev = 200, int microsteps = 32);

  void begin();
  void moveAngleTo(double targetAngle);
  void update();
  void setMaxSpeed(float stepsPerSec);
  void setAcceleration(float stepsPerSec2);
  void reset();
  void enable();
  void disable();
  bool isRunning();
  long currentPosition();
  double currentAngle();

private:
  AccelStepper motor_;
  uint8_t DIR_, STEP_;
  int stepsPerRev_, microsteps_;
  double currentAngle_;
  double continuousTargetAngle_;

  float maxSpeed_;
  float acceleration_;
  float currentSpeed_;
  unsigned long lastUpdateTime_;

  long angleToSteps(double angle) const;
};
