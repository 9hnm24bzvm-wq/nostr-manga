use serde::{Serialize, Deserialize};
use std::io::{self, Read};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}
fn main() { let mut input=String::new(); io::stdin().read_to_string(&mut input).unwrap(); let event: Event = serde_json::from_str(&input).unwrap(); println!("{}",serde_json::to_value(&event).unwrap()); }
