use std::io::{self, Write};
use std::process;

fn main() {
    // Constants from biophysical data
    let r: f64 = 0.3; // growth rate
    let k: f64 = 150.0; // carrying capacity in millions
    let alpha: f64 = 0.01; // Varroa impact
    let beta: f64 = 0.0005; // Pesticide impact
    let gamma: f64 = 0.2; // Nutrition impact
    let delta: f64 = 0.05; // Climate impact
    let epsilon: f64 = 0.4; // Intervention efficacy

    // Baseline stressors
    let v: f64 = 5.0;
    let p: f64 = 10.0;
    let n: f64 = 0.15;
    let c: f64 = 1.5;
    let i: f64 = 0.8; // Intervention intensity

    // Initial population
    let mut b: f64 = 100.0;
    let dt: f64 = 0.1; // time step in years
    let years: usize = 10;
    let steps: usize = (years as f64 / dt) as usize;

    println!("Year\tPopulation (millions)");
    println!("0\t{:.2}", b);

    for step in 1..=steps {
        let db = r * b * (1.0 - b / k) - alpha * v * b - beta * p * b - gamma * n * b - delta * c * b + epsilon * i * b;
        b += db * dt;

        if step % (1.0 / dt) as usize == 0 {
            println!("{}\t{:.2}", step / (1.0 / dt) as usize, b);
        }

        // Safety invariant: Abort if model predicts extinction (representationally impossible harm)
        if b <= 0.0 {
            eprintln!("Error: Model predicts extinction. Adjust parameters.");
            process::exit(1);
        }
    }

    // Steady-state calculation
    let steady = k * (1.0 - (alpha * v + beta * p + gamma * n + delta * c) / r + epsilon * i);
    println!("\nSteady-state population: {:.2} million colonies", steady);
}
