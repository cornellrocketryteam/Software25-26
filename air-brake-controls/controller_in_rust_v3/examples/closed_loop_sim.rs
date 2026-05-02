use controller_in_rust_v3::{AirbrakeSystem, Phase, SensorInput};
use controller_in_rust_v3::constants::{G, MASS_KG, TARGET_APOGEE_M};
use controller_in_rust_v3::apogee::{air_density, cd_a_total};
use std::thread::sleep;
use std::time::Duration;

const DT: f32 = 0.05; // 20 Hz loop

fn main() {
    let mut system = AirbrakeSystem::new();
    let mut time = 0.0;
    
    // Initial burnout conditions
    // Set intentionally high to ensure we overshoot and force airbrakes to deploy
    let mut alt = 800.0;
    let mut vel = 240.0; 
    
    // Roughly 1 atm ground pressure
    let ground_pressure = 101325.0; 
    
    println!("Closed-Loop Flight Simulator");
    println!("Target Apogee: {:.1} m", TARGET_APOGEE_M);
    println!("{:<8} | {:<8} | {:<8} | {:<10} | {:<15} | {:<10}", 
        "Time(s)", "Alt(m)", "Vel(m/s)", "Deploy %", "Pred Apogee(m)", "Drag(N)");
    println!("--------------------------------------------------------------------------------");
    
    while vel > 0.0 {
        let input = SensorInput {
            time,
            altitude: alt,
            vel_d: -vel,
            reference_pressure: ground_pressure,
            gyro_x: 0.0,
            gyro_y: 0.0,
            gyro_z: 0.0,
            accel_x: 0.0,
            accel_y: 0.0,
            accel_z: 0.0,
            phase: Phase::Coast,
        };
        
        let output = system.execute(&input);
        let deployment = output.deployment;
        
        // --- CLOSED-LOOP PHYSICS ENGINE ---
        // Recalculate physical parameters exactly as real life would dictate
        let rho = air_density(alt, ground_pressure);
        let cd_a = cd_a_total(deployment);
        
        // Drag = 1/2 * rho * v^2 * CdA
        let drag_force = 0.5 * rho * vel * vel * cd_a;
        
        // Acceleration (downward is negative)
        // a = -g - (drag / mass)
        let accel = -G - (drag_force / MASS_KG);
        
        // Log telemetry
        println!("{:<8.2} | {:<8.1} | {:<8.1} | {:<10.2} | {:<15.1} | {:<10.1}", 
            time, alt, vel, deployment * 100.0, output.predicted_apogee, drag_force);
            
        // Forward Euler integration
        let vel_next = vel + accel * DT;
        let alt_next = alt + vel * DT + 0.5 * accel * DT * DT;
        
        vel = vel_next;
        alt = alt_next;
        time += DT;
        
        // Uncomment to run exactly in real-time
        // sleep(Duration::from_millis(50));
    }
    
    println!("--------------------------------------------------------------------------------");
    println!("Simulation Complete!");
    println!("Actual Apogee Reached: {:.1} m", alt);
    println!("Target Apogee: {:.1} m", TARGET_APOGEE_M);
    let error = alt - TARGET_APOGEE_M;
    println!("Error: {:.1} m", error);
}
