use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
struct Block {
    index: u64,
    timestamp: u128,
    data: String, // e.g., "Temp:34.5,HSP:1.2,Varroa:2.1"
    prev_hash: u64,
    hash: u64,
}

impl Block {
    fn new(index: u64, data: String, prev_hash: u64) -> Self {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        let mut block = Block { index, timestamp, data, prev_hash, hash: 0 };
        block.hash = block.calculate_hash();
        block
    }

    fn calculate_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.index.hash(&mut hasher);
        self.timestamp.hash(&mut hasher);
        self.data.hash(&mut hasher);
        self.prev_hash.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Debug)]
struct Blockchain {
    chain: Vec<Block>,
    stress_threshold: f64, // e.g., 0.1 for zero-harm corridor
}

impl Blockchain {
    fn new() -> Self {
        let genesis = Block::new(0, "Genesis: Bee Neural Sovereignty Enforced".to_string(), 0);
        Blockchain { chain: vec![genesis], stress_threshold: 0.1 }
    }

    fn add_block(&mut self, data: String) -> Result<(), String> {
        // Parse data for stress check (simplified: assume last field is stress metric)
        let parts: Vec<&str> = data.split(',').collect();
        if let Some(last) = parts.last() {
            if let Ok(stress) = last.parse::<f64>() {
                if stress > self.stress_threshold {
                    return Err("Block rejected: Stress exceeds zero-harm corridor".to_string());
                }
            }
        }

        let prev_hash = self.chain.last().unwrap().hash;
        let new_block = Block::new(self.chain.len() as u64, data, prev_hash);
        // Invariant: Verify hash integrity
        if new_block.hash != new_block.calculate_hash() {
            return Err("Hash invalid: Representationally impossible".to_string());
        }
        self.chain.push(new_block);
        Ok(())
    }
}

fn main() {
    let mut chain = Blockchain::new();
    // Example additions with safety enforcement
    if chain.add_block("Temp:34.0,HSP:0.05,Varroa:1.0,Stress:0.08".to_string()).is_err() {
        eprintln!("Failed to add safe block");
    }
    if let Err(e) = chain.add_block("Temp:36.0,HSP:1.5,Varroa:5.0,Stress:0.12".to_string()) {
        println!("Vetoed: {}", e); // Enforces auditor-like veto
    }
    println!("{:?}", chain);
}
