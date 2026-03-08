#include "StepperMotor.h"
#include <cmath>

StepperMotor::StepperMotor(uint8_t DIR, uint8_t STEP, uint8_t EN,
                           int stepsPerRev, int microsteps)
    : DIR_(DIR), STEP_(STEP), stepsPerRev_(stepsPerRev),
      microsteps_(microsteps), currentAngle_(0.0), continuousTargetAngle_(0.0),
      maxSpeed_(1000.0f), acceleration_(500.0f), currentSpeed_(0.0f),
      lastUpdateTime_(0), motor_(AccelStepper::DRIVER, STEP, DIR) {
  motor_.setMaxSpeed(maxSpeed_);
  motor_.setAcceleration(acceleration_);

  if (EN != 255) {
    motor_.setEnablePin(EN);
    // Default active low enable for most stepper
    // drivers (e.g. TMC, A4988)
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
  return (long)std::round(angle * stepsPerDegree);
}

void StepperMotor::moveAngleTo(double targetAngle) {
  // Constrain incoming target to [0, 360)
  targetAngle = fmod(targetAngle, 360.0);
  if (targetAngle < 0) {
    targetAngle += 360.0;
  }

  // Find the constrained value of our continuous
  // absolute target
  double currentTargetMod = fmod(continuousTargetAngle_, 360.0);
  if (currentTargetMod < 0) {
    currentTargetMod += 360.0;
  }

  // Calculate shortest path difference
  double difference = targetAngle - currentTargetMod;
  if (difference > 180.0) {
    difference -= 360.0;
  } else if (difference < -180.0) {
    difference += 360.0;
  }

  // Accumulate the absolute continuous angle
  continuousTargetAngle_ += difference;

  // The motor is no longer directly commanded by moveTo() here.
  // Instead, the update() function will continuously track
  // continuousTargetAngle_ using a velocity profile.
  // The currentAngle_ is updated to reflect the *intended* target.
  currentAngle_ = targetAngle;
}

bool StepperMotor::isRunning() {
  return std::abs(currentSpeed_) > 0.1f ||
         motor_.currentPosition() != angleToSteps(continuousTargetAngle_);
}

long StepperMotor::currentPosition() { return motor_.currentPosition(); }

double StepperMotor::currentAngle() { return currentAngle_; }

void StepperMotor::update() {
  long targetSteps = angleToSteps(continuousTargetAngle_);
  long error = targetSteps - motor_.currentPosition();

  float targetSpeed = 0.0f;
  long absError = std::abs(error);

  if (absError > 0) {
    float accel_f = acceleration_;
    // Time-optimal deceleration tracking curve: v = sqrt(2 * a * e)
    targetSpeed = std::sqrt(2.0f * accel_f * absError);

    // Linearize near zero to smooth out micro-steps and prevent buzzing (limit
    // cycling)
    const float threshold = 5.0f;
    if (absError < threshold) {
      float v_thresh = std::sqrt(2.0f * accel_f * threshold);
      targetSpeed = ((float)absError / threshold) * v_thresh;
    }

    if (error < 0) {
      targetSpeed = -targetSpeed;
    }
  }

  if (targetSpeed > maxSpeed_)
    targetSpeed = maxSpeed_;
  if (targetSpeed < -maxSpeed_)
    targetSpeed = -maxSpeed_;

  unsigned long now = micros();
  if (lastUpdateTime_ == 0)
    lastUpdateTime_ = now;

  unsigned long dt_micros = now - lastUpdateTime_;
  lastUpdateTime_ = now;

  float dt = dt_micros / 1000000.0f;
  if (dt > 0.1f)
    dt = 0.1f;

  // Kinematic acceleration limits to strictly enforce max acceleration
  if (currentSpeed_ < targetSpeed) {
    currentSpeed_ += acceleration_ * dt;
    if (currentSpeed_ > targetSpeed) {
      currentSpeed_ = targetSpeed;
    }
  } else if (currentSpeed_ > targetSpeed) {
    currentSpeed_ -= acceleration_ * dt;
    if (currentSpeed_ < targetSpeed) {
      currentSpeed_ = targetSpeed;
    }
  }

  // Pass the calculated speed directly to AccelStepper and step if needed
  motor_.setSpeed(currentSpeed_);
  motor_.runSpeed();
}

void StepperMotor::setMaxSpeed(float stepsPerSec) {
  maxSpeed_ = stepsPerSec;
  motor_.setMaxSpeed(stepsPerSec);
}

void StepperMotor::setAcceleration(float stepsPerSec2) {
  acceleration_ = stepsPerSec2;
  motor_.setAcceleration(stepsPerSec2);
}

void StepperMotor::reset() {
  motor_.setCurrentPosition(0);
  currentAngle_ = 0.0;
  continuousTargetAngle_ = 0.0;
  currentSpeed_ = 0.0f;
  lastUpdateTime_ = 0;
}