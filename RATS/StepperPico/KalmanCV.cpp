#include "KalmanCV.h"
#include <cstring>
#include <cmath>

static inline void copy6(const double A[6][6], double B[6][6]) {
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            B[i][j] = A[i][j];
}

KalmanCV::KalmanCV()
    : last_t(0.0), accelVar(10.0) {
    std::memset(&x, 0, sizeof(x));
    zeroP();
}

void KalmanCV::setAccelVariance(double q) {
    accelVar = q;
}

void KalmanCV::zeroP() {
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            P[i][j] = 0.0;
}

void KalmanCV::init(double t0, const Vec3 &pos, double posVar, double velVar) {
    last_t = t0;

    x.d[0] = pos.x;
    x.d[1] = pos.y;
    x.d[2] = pos.z;
    x.d[3] = x.d[4] = x.d[5] = 0.0;

    zeroP();
    P[0][0] = P[1][1] = P[2][2] = posVar;
    P[3][3] = P[4][4] = P[5][5] = velVar;
}

void KalmanCV::predict(double t) {
    double dt = t - last_t;
    if (dt <= 0.0) return;

    double F[6][6] = {0};
    for (int i = 0; i < 3; i++) {
        F[i][i] = 1.0;
        F[i][i + 3] = dt;
        F[i + 3][i + 3] = 1.0;
    }

    // State prediction
    State6 xnew{};
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            xnew.d[i] += F[i][j] * x.d[j];

    // Process noise (continuous white acceleration)
    double dt2 = dt * dt;
    double dt3 = dt2 * dt;

    double q11 = accelVar * dt3 / 3.0;
    double q13 = accelVar * dt2 / 2.0;
    double q33 = accelVar * dt;

    double Q[6][6] = {0};
    for (int i = 0; i < 3; i++) {
        Q[i][i] = q11;
        Q[i][i + 3] = q13;
        Q[i + 3][i] = q13;
        Q[i + 3][i + 3] = q33;
    }

    double FP[6][6] = {0};
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            for (int k = 0; k < 6; k++)
                FP[i][j] += F[i][k] * P[k][j];

    double Pnew[6][6] = {0};
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            for (int k = 0; k < 6; k++)
                Pnew[i][j] += FP[i][k] * F[j][k];

    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            Pnew[i][j] += Q[i][j];

    x = xnew;
    copy6(Pnew, P);
    last_t = t;
}

void KalmanCV::updatePosition(double t, const Vec3 &pos, double measVar) {
    predict(t);

    double y[3] = {
        pos.x - x.d[0],
        pos.y - x.d[1],
        pos.z - x.d[2]
    };

    double S[3][3];
    for (int i = 0; i < 3; i++) {
        for (int j = 0; j < 3; j++)
            S[i][j] = P[i][j];
        S[i][i] += measVar;
    }

    double det =
        S[0][0]*(S[1][1]*S[2][2]-S[1][2]*S[2][1]) -
        S[0][1]*(S[1][0]*S[2][2]-S[1][2]*S[2][0]) +
        S[0][2]*(S[1][0]*S[2][1]-S[1][1]*S[2][0]);

    if (fabs(det) < 1e-9) return;

    double invS[3][3];
    invS[0][0] =  (S[1][1]*S[2][2]-S[1][2]*S[2][1]) / det;
    invS[0][1] = -(S[0][1]*S[2][2]-S[0][2]*S[2][1]) / det;
    invS[0][2] =  (S[0][1]*S[1][2]-S[0][2]*S[1][1]) / det;
    invS[1][0] = -(S[1][0]*S[2][2]-S[1][2]*S[2][0]) / det;
    invS[1][1] =  (S[0][0]*S[2][2]-S[0][2]*S[2][0]) / det;
    invS[1][2] = -(S[0][0]*S[1][2]-S[0][2]*S[1][0]) / det;
    invS[2][0] =  (S[1][0]*S[2][1]-S[1][1]*S[2][0]) / det;
    invS[2][1] = -(S[0][0]*S[2][1]-S[0][1]*S[2][0]) / det;
    invS[2][2] =  (S[0][0]*S[1][1]-S[0][1]*S[1][0]) / det;

    double K[6][3] = {0};
    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 3; j++)
            for (int k = 0; k < 3; k++)
                K[i][j] += P[i][k] * invS[k][j];

    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 3; j++)
            x.d[i] += K[i][j] * y[j];

    // Joseph-form covariance update
    double Pnew[6][6] = {0};
    for (int i = 0; i < 6; i++) {
        for (int j = 0; j < 6; j++) {
            double s = P[i][j];
            for (int k = 0; k < 3; k++)
                s -= K[i][k] * P[k][j];
            Pnew[i][j] = s;
        }
    }

    for (int i = 0; i < 6; i++)
        for (int j = 0; j < 6; j++)
            for (int k = 0; k < 3; k++)
                Pnew[i][j] += K[i][k] * measVar * K[j][k];

    copy6(Pnew, P);
}

State6 KalmanCV::predictFuture(double tau) const {
    State6 s{};
    for (int i = 0; i < 3; i++) {
        s.d[i] = x.d[i] + tau * x.d[i + 3];
        s.d[i + 3] = x.d[i + 3];
    }
    return s;
}
