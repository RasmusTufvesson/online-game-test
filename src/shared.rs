use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub enum Packet {
    Joined(u32), // id
    Left(u32), // id
    Move(u32, Vec2), // id, new pos
    OnJoin(u32, Vec<(u32, Vec2)>), // id, (id, pos)
    Sync(Vec<(u32, Vec2)>), // id, pos
}

impl Packet {
    pub fn to_slice(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        bincode::deserialize(slice).unwrap()
    }
}