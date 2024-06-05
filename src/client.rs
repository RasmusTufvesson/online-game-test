use std::{collections::HashMap, io::{Read, Write}, net::TcpStream};

use crate::shared::Packet;
use macroquad::prelude::*;

mod shared;

#[macroquad::main("Online test")]
async fn main() {
    println!("Connecting");
    let stream = TcpStream::connect("127.0.0.1:6666").unwrap();
    let mut socket = Connection { stream, buf: [0; 1024] };
    let mut players: HashMap<u32, Vec2> = HashMap::new();
    println!("Wait for packet");
    let id = if let Some(Packet::OnJoin(id, positions)) = socket.recv_packet() {
        for (p_id, pos) in positions {
            players.insert(p_id, pos);
        }
        id
    } else {
        panic!("Could not get on join packet")
    };
    println!("{:?}", players);
    socket.set_no_block();
    let mut player = Player { position: vec2(50.0, 50.0), speed: 250.0, id };
    loop {
        let delta_time = get_frame_time();

        clear_background(BLACK);

        player.process_movement(&mut socket, delta_time);

        for pos in players.values() {
            draw_circle(pos.x, pos.y, 15.0, BLUE);
        }
        draw_circle(player.position.x, player.position.y, 15.0, BLUE);

        while let Some(packet) = socket.recv_packet() {
            match packet {
                Packet::Joined(id) => {
                    let _ = players.insert(id, vec2(50.0, 50.0));
                }
                Packet::Left(id) => {
                    let _ = players.remove(&id);
                }
                Packet::Move(id, new_pos) => {
                    if let Some(pos) = players.get_mut(&id) {
                        *pos = new_pos;
                    }
                }
                _ => unreachable!()
            }
        }

        next_frame().await
    }
}

struct Player {
    position: Vec2,
    speed: f32,
    id: u32,
}

impl Player {
    fn process_movement(&mut self, socket: &mut Connection, delta_time: f32) {
        let mut diff = Vec2::ZERO;
        if is_key_down(KeyCode::W) {
            diff.y -= 1.0;
        }
        if is_key_down(KeyCode::S) {
            diff.y += 1.0;
        }
        if is_key_down(KeyCode::A) {
            diff.x -= 1.0;
        }
        if is_key_down(KeyCode::D) {
            diff.x += 1.0;
        }
        let diff = diff.normalize_or_zero() * self.speed * delta_time;
        self.position += diff;
        socket.send_packet(Packet::Move(self.id, self.position));
    }
}

struct Connection {
    stream: TcpStream,
    buf: [u8; 1024],
}

impl Connection {
    fn send_packet(&mut self, packet: Packet) {
        let data = packet.to_slice();
        self.stream.write_all(data.as_slice()).expect("Couldnt send packet");
    }

    fn recv_packet(&mut self) -> Option<Packet> {
        match self.stream.read(&mut self.buf) {
            Ok(_) => {
                let packet = Packet::from_slice(&self.buf);
                self.buf.fill(0);
                Some(packet)
            }
            Err(_) => None,
        }
    }

    fn set_no_block(&mut self) {
        self.stream.set_nonblocking(true).unwrap();
    }
}