#pragma once
#include "LLA.h"

class GeoMath {
    public:
    static Azimuth_El compute_angle_el(const LLA& rats, const LLA& rocket);
};