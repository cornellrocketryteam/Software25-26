#include "KalmanCV.h"
#include <cstring>
#include <cmath>

static inline void copy6(const double A[6][6], double B[6][6]) {
    for (int i = 0; i < 6; i++) {
        for (int j = 0; j < 6; j++) {
            B[i][j] = A[i][j];
        }
    }
}

KalmanCV::KalmanCV() {
    last_t = 0.0;
    std::memset(&x, 0, sizeof(x));
    zeroP();
}

void KalmanCV::zeroP() {
        for (int i = 0; i < 6; i++) {
            for (int j = 0; j < 6; j++) {
                P[i][j] = 0.0;
            }
    }
}

void KalmanCV::init(double t0, const Vec3 &pos, double posVar, double velVar) {
  last_t = t0;
  x.d[0] = pos.x; x.d[1] = pos.y; x.d[2] = pos.z;
  x.d[3] = x.d[4] = x.d[5] = 0.0;
  zeroP();
  P[0][0] = posVar; P[1][1] = posVar; P[2][2] = posVar;
  P[3][3] = velVar; P[4][4] = velVar; P[5][5] = velVar;
}

void KalmanCV::predict(double t) {
  double dt = t - last_t;
  if (dt <= 0.0) return;
  double F[6][6]; 
  for (int i = 0; i < 6; i++) {
    for (int j = 0; j < 6; j++) {
        F[i][j] = 0.0;
    }
  }
  for (int i = 0; i < 3; i++) {
    F[i][i] = 1.0;
    F[i][i + 3] = dt;
    F[i + 3][i + 3] = 1.0;
  }
  State6 xnew;
  for (int i = 0; i < 6; i++) {
    xnew.d[i] = 0.0;
    for (int j = 0; j < 6; j++) {
        xnew.d[i] += F[i][j] * x.d[j];
    } 
  }

  double qa = 10.0;
  double Q[6][6]; 
  for (int i = 0; i < 6; i++) {
    for (int j = 0; j < 6; j++) {
        Q[i][j] = 0.0;
    }
  }
  
  double dt2 = dt * dt, dt3 = dt2 * dt;
  double q11 = qa * dt3/3.0;
  double q13 = qa * dt2/2.0;
  double q33 = qa * dt;
  for (int i = 0; i < 3; i++){
    Q[i][i] = q11;
    Q[i][i + 3] = q13;
    Q[i + 3][i] = q13;
    Q[i + 3][i + 3] = q33;
  }

  double FP[6][6];
  for (int i = 0; i < 6; i++) {
    for (int j = 0; j < 6; j++) {
    double s = 0.0;
        for (int k = 0;k < 6; k++) {
            s += F[i][k]*P[k][j];
        }
    FP[i][j] = s;
    }
  }

  double Pnew[6][6];
  for (int i = 0; i < 6; i++) { 
    for (int j = 0; j < 6; j++) {
    double s=0.0;
    for (int k = 0; k < 6; k++) {
        s += FP[i][k]*F[j][k];
    }
    Pnew[i][j] = s + Q[i][j];
    }
  }

  x = xnew;
  copy6(Pnew, P);
  last_t = t;
}

void KalmanCV::updatePosition(const Vec3 &pos, double measVar) {
  double y[3];
  y[0] = pos.x - x.d[0];
  y[1] = pos.y - x.d[1];
  y[2] = pos.z - x.d[2];

  double S[3][3];
  for (int i = 0; i < 3; i++) { 
    for (int j = 0; j < 3; j++) {
        S[i][j] = P[i][j]; 
    }
    S[0][0] += measVar; S[1][1] += measVar; S[2][2] += measVar;
  }

  double det = S[0][0] * (S[1][1]*S[2][2]-S[1][2] * S[2][1])
             - S[0][1] * (S[1][0]*S[2][2]-S[1][2] * S[2][0])
             + S[0][2] * (S[1][0]*S[2][1]-S[1][1] * S[2][0]);
  if (fabs(det) < 1e-12) {
    return;
  }
  double invS[3][3];
  invS[0][0] =  (S[1][1] * S[2][2] - S[1][2] * S[2][1]) / det;
  invS[0][1] = -(S[0][1] * S[2][2] - S[0][2] * S[2][1]) / det;
  invS[0][2] =  (S[0][1] * S[1][2] - S[0][2] * S[1][1]) / det;
  invS[1][0] = -(S[1][0] * S[2][2] - S[1][2] * S[2][0]) / det;
  invS[1][1] =  (S[0][0] * S[2][2] - S[0][2] * S[2][0]) / det;
  invS[1][2] = -(S[0][0] * S[1][2] - S[0][2] * S[1][0]) / det;
  invS[2][0] =  (S[1][0] * S[2][1] - S[1][1] * S[2][0]) / det;
  invS[2][1] = -(S[0][0] * S[2][1] - S[0][1] * S[2][0]) / det;
  invS[2][2] =  (S[0][0] * S[1][1] - S[0][1] * S[1][0]) / det;

  double K[6][3];
  for (int i = 0; i < 6; i++) {
    for (int j = 0; j < 3; j++) {
        double s = 0.0;
        for (int k = 0; k < 3; k++) {
            s += P[i][k] * invS[k][j]; 
        }
        K[i][j] = s;
    }
  }

  for (int i = 0; i < 6; i++) {
    double s = 0.0;
    for (int j = 0; j < 3; j++) {
        s += K[i][j] * y[j]; 
    }
    x.d[i] += s;
  }

  double KH[6][6]; 
  for (int i = 0; i < 6; i++) {
    for (int j = 0; j < 6; j++) {
        KH[i][j]=0.0;
    }
  }
  for (int i = 0; i < 6; i++) { 
    for (int j = 0; j < 3; j++) {
        KH[i][j] = K[i][j];
    }
  }
  double IminusKH[6][6];
  for (int i = 0; i < 6; i++) { 
    for (int j = 0; j < 6; j++) { 
        IminusKH[i][j] = (i==j?1.0:0.0) - KH[i][j];
    }
  }
  double Pnew[6][6];
  for (int i = 0; i < 6; i++) { 
    for (int j = 0; j < 6; j++) {
        double s = 0.0;
        for (int k = 0; k < 6; k++) {
            s += IminusKH[i][k] * P[k][j];
        }
        Pnew[i][j] = s;
    }
  }
  copy6(Pnew, P);
}

State6 KalmanCV::predictFuture(double tau) const {
  double F[6][6]; 
  for (int i = 0; i < 6; i++) { 
    for (int j = 0; j < 6; j++) {
        F[i][j] = 0.0; 
    }
  }
  for (int i = 0; i < 3; i++) { 
    F[i][i] = 1.0; 
    F[i][i + 3] = tau; 
    F[i + 3][i + 3] = 1.0; 
  }
  State6 s; 
  for (int i = 0; i < 6; i++) { 
    s.d[i] = 0.0; 
    for (int j = 0; j < 6; j++) { 
        s.d[i] += F[i][j] * x.d[j]; 
    }
  }
  return s;
}
