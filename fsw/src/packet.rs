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
}

impl Packet {
    pub fn init_empty() -> Self {
        Self {
            flight_mode: 0,
            pressure: 0.0,
            temp: 0.0,
            altitude: 0.0,
            latitude: 0.0,
            longitude: 0.0,
            num_satellites: 0,
            timestamp: 0.0,
        }
    }
}
