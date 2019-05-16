
use crate::structs;
use structs::StatePtr;
use structs::Player;

use std::env;
use std::net::{TcpListener, TcpStream};
use std::io::prelude::*;
use std::io::BufReader;
use std::time::{Duration, Instant};

use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

use std::path::Path;
use std::fs::File;

const NET_FILENAME: &str = "net_setup.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NetSetup {
    transfer_ms: u64,
    timeout_ms: u64,
    packet_ttl: u32,
    non_blocking: bool,
    nodelay: bool,
}

impl NetSetup {
    pub fn from_file<T: AsRef<Path>>(filename: T) -> std::io::Result<NetSetup> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut data: NetSetup = serde_json::from_reader(reader)?;
        data.transfer_ms = std::cmp::max(data.transfer_ms, 1);
        Ok(data)
    }
    
    #[allow(unused_must_use)]
    pub fn configure_stream(&self, stream: &mut TcpStream) {
        stream.set_nodelay(self.nodelay);
        
        let timeout: Option<Duration> = 
            if self.timeout_ms > 0 { 
                Some(Duration::from_millis(self.timeout_ms)) 
            } else { 
                None 
            };
        
        stream.set_read_timeout(timeout);
        stream.set_write_timeout(timeout);
        stream.set_ttl(self.packet_ttl);
        stream.set_nonblocking(self.non_blocking);
    }

    pub fn write_default<T: AsRef<Path>>(filename: T) -> NetSetup {
        match File::create(filename) {
            Ok(file) => {
                let net = NetSetup::default();
                // We don't care if this fails
                let _ = serde_json::to_writer_pretty(file, &net);
                return net
            }
            _ => return NetSetup::default()
        }
    }
}

impl Default for NetSetup {
    fn default() -> NetSetup {
        NetSetup {
            transfer_ms: 33,
            timeout_ms: 1000,
            packet_ttl: 60,
            non_blocking: false,
            nodelay: true,
        }
    }
}

fn block_for_next(time: Instant, transfer_ms: u64) -> Instant {
    let elapsed = time.elapsed().as_millis() as u64;
    let wait_ms: u64 = 
        if transfer_ms > elapsed {
            transfer_ms - elapsed
        } else { 0 };

    std::thread::sleep(Duration::from_millis(wait_ms));
    Instant::now()
}

pub fn network_main(stateptr: &mut StatePtr) { 
    let mut is_server = false;

    let mut args: std::vec::Vec<String> = env::args().collect();
    if args.len() <= 2 {
        is_server = true;
    }
    let is_server = is_server;

    if !is_server {
        client_main(stateptr, &mut args[2]).expect("Client thread paniced.");
    } else {
        server_main(stateptr).expect("Server thread paniced.");
    }
}

/// Attempts to send the struct in the stream.
fn send_struct<T: Serialize>(stream: &mut TcpStream, data: T) -> usize {
    let bin = bincode::serialize(&data).expect("Failed to serialize.");
    let _ = stream.write_all(&bin[..]);
    bin.len()
}


/// Runs the given Function with the Deserialized struct. 
/// Intended to edit a mutable state capture.
fn recv_update<T: DeserializeOwned>(stream: &mut TcpStream, function: impl Fn(T)) {
    let read_buf = BufReader::new(stream.try_clone().expect("Failed to clone stream."));
    
    let data = bincode::deserialize_from::<_, T>(read_buf);
    if let Ok(data) = data {
        function(data);
    }
}

fn client_main(stateptr: &mut StatePtr, server_addres: &mut String) -> std::io::Result<()> {
    
    let mut recv_stream = TcpStream::connect(format!("{}:9942", server_addres))?;
    let mut send_stream = TcpStream::connect(format!("{}:9949", server_addres))?;

    let net = NetSetup::from_file(NET_FILENAME).unwrap_or_else(|_| NetSetup::write_default(NET_FILENAME) );
    net.configure_stream(&mut recv_stream);
    net.configure_stream(&mut send_stream);

    println!("Client connecting! Transfer rate: {:?}ms", net.transfer_ms);

    
    stateptr.state.lock().unwrap().local_player_index = 1;
    

    let ptr = stateptr.get_ref();
    let net_copy = net.clone();
    std::thread::spawn(move || {
        let net = net_copy;
        println!("Recv thread.");
        let mut timer = Instant::now();    
        loop {
            timer = block_for_next(timer, net.transfer_ms);

            recv_update(&mut recv_stream, |data: structs::NetFromServer| {
                let mut state = ptr.state.lock().unwrap();
                data.update_main_state(&mut state);
            });
        }
    });

    let ptr = stateptr.get_ref();
    std::thread::spawn(move || {
        let mut timer = Instant::now();    
        loop {
            timer = block_for_next(timer, net.transfer_ms);

            let input_data;
            {
                let state = ptr.state.lock().unwrap();
                input_data = state.local_input.clone();
            }

            send_struct(&mut send_stream, input_data);
        }
    });  
    Ok(())
}

fn server_sender(mut stream: TcpStream, stateptr: StatePtr, transfer_ms: u64) {
    let mut timer = Instant::now();
    let mut max_packet = 0 as usize;

    loop {
        timer = block_for_next(timer, transfer_ms);

        let mut net_struct;
        {
            let state = stateptr.state.lock().unwrap();
            net_struct = structs::NetFromServer::make_from_state(&state);
        }
        let size = send_struct(&mut stream, net_struct);

        if size > max_packet {
            println!("New max packet size: {}", size);
            max_packet = size;
        } 
    }
}

fn server_recver(mut stream: TcpStream, stateptr: StatePtr, transfer_ms: u64) -> std::io::Result<()> {
    let player_index;
    {
        let mut state = stateptr.state.lock().unwrap();
        state.players.push(Player::create());
        player_index = state.players.len() - 1;
    }

    let mut timer = Instant::now();    
    loop {
        timer = block_for_next(timer, transfer_ms);
        
        recv_update(&mut stream, |data: structs::InputState| {
            match stateptr.state.lock() {
                Ok(ref mut state) => {
                    state.players[player_index].input = data;
                },
                Err(_) => {},
            }
        });
    }
}

fn server_main(stateptr: &mut StatePtr) -> std::io::Result<()> {
    let send_lstener = TcpListener::bind("0.0.0.0:9942")?;
    let recv_listener = TcpListener::bind("0.0.0.0:9949")?;

    let net = NetSetup::from_file(NET_FILENAME).unwrap_or_else(|_| NetSetup::write_default(NET_FILENAME));

    println!("Server!");
    println!("Listening for connections.... Transfer rate: {:?}ms", net.transfer_ms);

    let mut ptr = stateptr.get_ref();
    let net_copy = net.clone();

    let _ = std::thread::Builder::new().name("server listener sender".into()).spawn(move || {
        let net = net_copy;
        for listen_result in send_lstener.incoming() {
            let this_listen_ref = ptr.get_ref();
            let mut stream = listen_result.expect("Server Sender Thread Failed.");
            net.configure_stream(&mut stream);
            println!("Client Connected: {:?}", stream.peer_addr());
            server_sender(stream, this_listen_ref, net.transfer_ms);
        }
    });

    let mut ptr = stateptr.get_ref();
    let _ = std::thread::Builder::new().name("server listener recver".into()).spawn(move || {
        for listen_result in recv_listener.incoming() {
            let this_listen_ref = ptr.get_ref();
            let mut stream = listen_result.expect("Server Recv Thread Failed.");
            net.configure_stream(&mut stream);
            server_recver(stream, this_listen_ref, net.transfer_ms).expect("Server Recv Thread Failed.");
        }
    });  

    Ok(())
}