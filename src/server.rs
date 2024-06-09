use glam::{vec2, Vec2};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, RwLock};
use tokio::time::Instant;
use std::time::Duration;
use std::{io, collections::HashMap, error::Error, net::SocketAddr, sync::Arc};
use crate::shared::Packet;

mod shared;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let state = Arc::new(RwLock::new(Shared::new()));

    let addr = "127.0.0.1:6666".to_string();

    // Bind a TCP listener to the socket address.
    //
    // Note that this is the Tokio TcpListener, which is fully async.
    let listener = TcpListener::bind(&addr).await?;

    println!("server running on {}", addr);

    loop {
        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;

        // Clone a handle to the `Shared` state for the new connection.
        let state = Arc::clone(&state);

        // Spawn our handler to be run asynchronously.
        tokio::spawn(async move {
            println!("accepted connection");
            if let Err(e) = process(state, stream, addr).await {
                println!("an error occurred; error = {:?}", e);
            }
        });
    }
}

type Tx = mpsc::UnboundedSender<Packet>;
type Rx = mpsc::UnboundedReceiver<Packet>;

struct Shared {
    peers: HashMap<SocketAddr, (Tx, Vec2, u32)>,
    next_id: u32,
}

struct Peer {
    rx: Rx,
    identifier: u32,
}

impl Shared {
    fn new() -> Self {
        Shared {
            peers: HashMap::new(),
            next_id: 0,
        }
    }

    fn broadcast(&self, sender: SocketAddr, message: Packet) {
        for peer in self.peers.iter() {
            if *peer.0 != sender {
                let _ = peer.1.0.send(message.clone());
            }
        }
    }
}

impl Peer {
    async fn new(
        state: Arc<RwLock<Shared>>,
        addr: SocketAddr,
    ) -> io::Result<Peer> {
        let (tx, rx) = mpsc::unbounded_channel();

        let mut state_w = state.write().await;
        let identifier = state_w.next_id;
        state_w.next_id += 1;
        state_w.peers.insert(addr, (tx, vec2(50.0, 50.0), identifier));

        Ok(Peer { rx, identifier })
    }
}

struct Connection {
    stream: TcpStream,
    buf: [u8; 1024],
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream, buf: [0; 1024] }
    }

    pub async fn recv(&mut self) -> Option<[u8; 1024]> {
        let val = match self.stream.read(&mut self.buf).await {
            Ok(_) => {
                Some(self.buf.clone())
            }
            _ => {
                None
            }
        };
        self.buf.fill(0);
        val
    }

    pub async fn recv_packet(&mut self) -> Option<Packet> {
        match self.recv().await {
            Some(val) => Some(Packet::from_slice(&val)),
            None => None,
        }
    }

    pub async fn send(&mut self, data: &[u8]) {
        self.stream
            .write_all(data)
            .await
            .expect("failed to write data to socket");
    }

    pub async fn send_packet(&mut self, packet: Packet) {
        let data = packet.to_slice();
        self.send(data.as_slice()).await;
    }
}

async fn process(
    state: Arc<RwLock<Shared>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {

    let mut socket = Connection::new(stream);

    let mut peer = Peer::new(state.clone(), addr).await?;
    
    println!("{} joined", peer.identifier);

    {
        let state_read = state.read().await;
        socket.send_packet(Packet::OnJoin(peer.identifier, state_read.peers.iter().map(|(_, (_, pos, id))| (*id, *pos)).filter(|(id, _)| id != &peer.identifier).collect())).await;
        state_read.broadcast(addr, Packet::Joined(peer.identifier));
    }

    let sleep = tokio::time::sleep_until(Instant::now() + Duration::from_millis(100));
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            Some(msg) = peer.rx.recv() => {
                socket.send_packet(msg).await;
            }
            result = socket.recv_packet() => match result {
                Some(msg) => {
                    let mut state_write = state.write().await;
                    match msg {
                        Packet::Move(_, pos) => {
                            state_write.peers.get_mut(&addr).unwrap().1 = pos;
                        }
                        _ => {}
                    }
                    state_write.broadcast(addr, msg);
                }
                None => break,
            },
            () = &mut sleep => {
                sleep.as_mut().reset(Instant::now() + Duration::from_millis(100));
                let state_read = state.read().await;
                socket.send_packet(Packet::Sync(state_read.peers.iter().map(|(_, (_, pos, id))| (*id, *pos)).filter(|(id, _)| id != &peer.identifier).collect())).await;
            }
        }
    }

    {
        let mut state_write = state.write().await;
        state_write.peers.remove(&addr);
        state_write.broadcast(addr, Packet::Left(peer.identifier));
    }

    println!("{} left", peer.identifier);

    Ok(())
}