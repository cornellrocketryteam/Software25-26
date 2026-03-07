#pragma once
#include "GeoMath.h"

struct State6 { double d[6]; };

class KalmanCV {
    public: 
        KalmanCV();

        void init(double t0, const Vec3 &pos, double posVar = 25.0, double velVar = 25.0);
        void predict(double t);
        void updatePosition(const Vec3 &pos, double measVar = 25.0);
        State6 predictFuture(double tau) const;
        State6 getState() const { return x; }

    private:
        double last_t;
        State6 x;
        double P[6][6];
        void zeroP();
};