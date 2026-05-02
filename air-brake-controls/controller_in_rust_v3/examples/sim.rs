use controller_in_rust_v3::{AirbrakeSystem, Phase, SensorInput};
use std::thread::sleep;
use std::time::Duration;

include!("sim_data.rs");

const DT: f32 = 0.05; // 20 Hz loop

fn main() {
    let mut system = AirbrakeSystem::new();
    let mut time = 0.0;
    let mut prev_alt = 0.0;
    let mut phase = Phase::Pad;
    
    // Roughly 1 atm ground pressure
    let ground_pressure = 101325.0; 
    
    println!("{:<8} | {:<8} | {:<8} | {:<10} | {:<15}", 
        "Time(s)", "Alt(m)", "Vel(m/s)", "Deploy %", "Pred Apogee(m)");
    println!("------------------------------------------------------------------");
    
    for &alt in TEST_ALTS_LST.iter() {
        let vel_up = (alt - prev_alt) / DT;
        let vel_d = -vel_up;
        
        // Simple state machine to emulate flight mode
        if time > 0.0 && alt > 10.0 && phase == Phase::Pad {
            phase = Phase::Boost;
        }
        // Transition to Coast phase after burnout (roughly 4-5 seconds)
        // or if upward velocity starts to dip significantly.
        if time > 5.0 && phase == Phase::Boost {
            phase = Phase::Coast;
        }

        let input = SensorInput {
            time,
            altitude: alt,
            vel_d,
            reference_pressure: ground_pressure,
            gyro_x: 0.0,
            gyro_y: 0.0,
            gyro_z: 0.0,
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: 0.0,
            phase,
        };
        
        let output = system.execute(&input);
        
        // Only print during coast to avoid spamming the pad/boost logs, since coast is when the airbrakes run
        if phase == Phase::Coast && vel_up > 0.0 {
            println!("{:<8.2} | {:<8.1} | {:<8.1} | {:<10.2} | {:<15.1}", 
                time, alt, vel_up, output.deployment * 100.0, output.predicted_apogee);
                
            // Sleep for 50ms to run it in exact real-time
            sleep(Duration::from_millis(50));
        }
            
        prev_alt = alt;
        time += DT;
    }
    
    println!("Simulation complete.");
}
