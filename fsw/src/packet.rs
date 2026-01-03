#[derive(Default)]
pub struct Packet {
    pub flight_mode: u32,
    // altimeter
    pub pressure: f32,
    pub temp: f32,
    pub altitude: f32,
    //gps
    pub latitude: f32,
    pub longitude: f32,
    pub num_satellites: u32,
    pub timestamp: f32,
    // magnetometer
    pub mag_x: f32,
    pub mag_y: f32,
    pub mag_z: f32,
    // imu - accelerometer (m/s²)
    pub accel_x: f32,
    pub accel_y: f32,
    pub accel_z: f32,
    // imu - gyroscope (°/s)
    pub gyro_x: f32,
    pub gyro_y: f32,
    pub gyro_z: f32,
}
