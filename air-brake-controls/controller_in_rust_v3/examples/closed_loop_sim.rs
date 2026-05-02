use controller_in_rust_v3::{AirbrakeSystem, Phase, SensorInput};
use controller_in_rust_v3::constants::{G, MASS_KG, TARGET_APOGEE_M};
use controller_in_rust_v3::apogee::{air_density, cd_a_total};
use std::env;

const DT: f32 = 0.05; // 20 Hz loop

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut alt = 800.0;
    let mut vel = 240.0;
    let json_output = args.iter().any(|a| a == "--json");
    
    let floats: Vec<f32> = args.iter().filter_map(|s| s.parse::<f32>().ok()).collect();
    if floats.len() >= 2 {
        alt = floats[0];
        vel = floats[1];
    }
    
    let ground_pressure = 101325.0; 
    
    if !json_output {
        println!("Closed-Loop Flight Simulator");
        println!("Target Apogee: {:.1} m", TARGET_APOGEE_M);
        println!("{:<8} | {:<8} | {:<8} | {:<10} | {:<15} | {:<10}", 
            "Time(s)", "Alt(m)", "Vel(m/s)", "Deploy %", "Pred Apogee(m)", "Drag(N)");
        println!("--------------------------------------------------------------------------------");
    } else {
        print!("[");
    }
    
    let mut system = AirbrakeSystem::new();
    let mut time = 0.0;
    let mut first_json = true;
    
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
        
        let rho = air_density(alt, ground_pressure);
        let cd_a = cd_a_total(deployment);
        let drag_force = 0.5 * rho * vel * vel * cd_a;
        let accel = -G - (drag_force / MASS_KG);
        
        if !json_output {
            println!("{:<8.2} | {:<8.1} | {:<8.1} | {:<10.2} | {:<15.1} | {:<10.1}", 
                time, alt, vel, deployment * 100.0, output.predicted_apogee, drag_force);
        } else {
            if !first_json { print!(","); }
            print!("{{\"time\":{:.3},\"alt\":{:.2},\"vel\":{:.2},\"deploy\":{:.2},\"pred_apogee\":{:.2},\"drag\":{:.2}}}",
                   time, alt, vel, deployment * 100.0, output.predicted_apogee, drag_force);
            first_json = false;
        }
            
        vel += accel * DT;
        alt += vel * DT;
        time += DT;
    }
    
    if !json_output {
        println!("--------------------------------------------------------------------------------");
        println!("Simulation Complete!");
        println!("Actual Apogee Reached: {:.1} m", alt);
        println!("Target Apogee: {:.1} m", TARGET_APOGEE_M);
        println!("Error: {:.1} m", alt - TARGET_APOGEE_M);
    } else {
        println!("]");
    }
}
