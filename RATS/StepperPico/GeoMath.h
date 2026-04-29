#pragma once
#include "LLA.h"
#include "AzEl.h"

// x = East, y = North, z = Up
struct Vec3 { double x; double y; double z; }; 

class GeoMath {
    public:
    static AzEl computeAzEl(const LLA &rats, const LLA &rocket);
    static Vec3 llatoENU(const LLA &observer, const LLA &target);
    static AzEl enuToAzEl(const Vec3 &enu);
};