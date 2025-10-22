#include "GeoMath.h"
#include <cmath>

Azimuth_El GeoMath::compute_angle_el(const LLA& rats, const LLA& rocket) {
    const double earthRadius = 6378137;
    const double eccentSqr = 6.69437999014e-3;
    const double PI = M_PI;

    double deg_to_rad (double deg) {
        return deg * PI / 180.0;
    };

    double rad_to_deg (double rad) {
        return rad * 180.0 / PI;
    };

    // LLA to ECEF (Cartesian Coords)
    void to_ECEF(double lat, double lon, double alt, double& x, double& y, double& z) {
        double sinLat = sin(lat), cosLat = cos(lat);
        double sinLon = sin(lon), cosLon = cos(lon);
        double N = earthRadius / sqrt(1 - eccentSqr * pow(sin(lat), 2));
        x = (N + alt) * cosLat * cosLon;
        y = (N + alt) * cosLat * sinLon;
        z = ((1 - eccentSqr) * N + alt) * sinLat;
    };

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

    double east = -sinLon * distx + cosLon * disty;
    double north = -cosLon * sinLat * distx - sinLat * sinLon * disty + cosLat * distz;
    double up = cosLat * cosLon * distx + cosLat * sinLon * disty + sinLat * distz;

    double range = sqrt(pow(east, 2) + pow(north, 2) + pow(up, 2));
    double horizontal = sqrt(pow(east, 2) + pow(north, 2));
    double azimuth = atan2(east, north);
    double elevation = atan2(up, horizontal);

    Azimuth_El result;
    result.azimuth = rad_to_deg(azimuth);
    result.elevation = rad_to_deg(elevation);
    result.range = range;
    return result;
}