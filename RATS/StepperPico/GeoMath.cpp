#include "GeoMath.h"
#include <cmath>

static constexpr double earthRadius = 6378137.0;
static constexpr double eccentSqr = 6.69437999014e-3;
static constexpr double PI = M_PI;

static inline double deg_to_rad(double d) { return d * PI / 180.0; }
static inline double rad_to_deg(double r) { return r * 180.0 / PI; }

static void to_ECEF(double lat, double lon, double alt, double& x, double& y, double& z) {
        double sinLat = sin(lat), cosLat = cos(lat);
        double sinLon = sin(lon), cosLon = cos(lon);
        double N = earthRadius / sqrt(1 - eccentSqr * pow(sin(lat), 2));
        x = (N + alt) * cosLat * cosLon;
        y = (N + alt) * cosLat * sinLon;
        z = ((1 - eccentSqr) * N + alt) * sinLat;
    };

Vec3 GeoMath::llatoENU(const LLA &rats, const LLA &rocket) {
    double lat0 = deg_to_rad(rats.lat);
    double lon0 = deg_to_rad(rats.lon);
    double lat1 = deg_to_rad(rocket.lat);
    double lon1 = deg_to_rad(rocket.lon);

    double x0, y0, z0, x1, y1, z1;
    to_ECEF(lat0, lon0, rats.alt, x0, y0, z0);
    to_ECEF(lat1, lon1, rocket.alt, x1, y1, z1);

    double distx = x1 - x0, disty = y1 - y0, distz = z1 - z0;

    double sinLat = sin(lat0), cosLat = cos(lat0);
    double sinLon = sin(lon0), cosLon = cos(lon0); 

    Vec3 enu;
    enu.x = -sinLon * distx + cosLon * disty;
    enu.y = -cosLon * sinLat * distx - sinLat * sinLon * disty + cosLat * distz;
    enu.z = cosLat * cosLon * distx + cosLat * sinLon * disty + sinLat * distz;
    return enu;
}

AzEl GeoMath::enuToAzEl(const Vec3 &enu) {
    AzEl result;
    double east = enu.x;
    double north = enu.y;
    double up = enu.z;
    double range = sqrt(pow(east, 2) + pow(north, 2) + pow(up, 2));
    double horizontal = sqrt(pow(east, 2) + pow(north, 2));
    double azimuth = atan2(east, north);
    double elevation = atan2(up, horizontal);
    result.azimuth = rad_to_deg(azimuth);
    result.elevation = rad_to_deg(elevation);
    result.range = range;
    return result;
}

AzEl GeoMath::computeAzEl(const LLA &rats, const LLA &rocket) {
    Vec3 enu = llatoENU(rats, rocket);
    return enuToAzEl(enu);
}