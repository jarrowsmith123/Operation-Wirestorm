use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SOURCE_ADDR: &str = "127.0.0.1:33333";
const DESTINATION_ADDR: &str = "127.0.0.1:44444";
const MAGIC: u8 = 0xCC;
const HEADER_SIZE: usize = 8;
const MAX_DESTINATIONS: usize = 100;    // Prevents number of destination clients potentially overwhelming server
const READ_TIMEOUT_SECS: u64 = 10;     // Prevent slow clients holding connection indefinitely


/// Represents a parsed CTMP message header
/// 1 magic byte = 0xCC
/// 1 byte of padding 0s
/// 2 bytes for the length of payload (16-bit limit provides implicit max payload size of 65,535 bytes)
/// 4 paddings bytes of 0s


// ============= CTMP Header structure ============

struct Header {
    magic: u8,
    padding1: u8, 
    length: u16,
    padding2: u32,
}

impl Header {
    // Creates a Header struct from an 8-byte buffer
    pub fn from_bytes(bytes: &[u8; HEADER_SIZE]) -> Self {
        // We need bigendian bytes due to specification
        let length: u16 = u16::from_be_bytes([bytes[2], bytes[3]]);
        let padding2: u32 = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        
        Header {
            magic: bytes[0],
            padding1: bytes[1],
            length,
            padding2: padding2,
        }
    }

    /// Validates the header fields against CTMP spec
    /// Magic byte must be 0xCC and both padding bytes should be filled with 0s
    /// Length is implicitly bounded by size of u16 
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC && self.padding1 == 0 && self.padding2 == 0
    }

    /// Returns the payload length as a usize for vec allocation
    pub fn payload_length(&self) -> usize {
        self.length as usize
    }
}


// ============== Main Server Logic ================

fn main() -> io::Result<()> {
    println!("Starting CTMP Proxy Server...");

    // This list of destination clients must be shared across multiple threads
    // so we store in a Vec and wrap it in an Arc Mutex
    let destinations: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::with_capacity(MAX_DESTINATIONS)));

    // ------------- Destination listener threads  ------------------
    let destinations_clone = Arc::clone(&destinations);
    let dest_listener = TcpListener::bind(DESTINATION_ADDR)?;
    println!("Listening for destination clients on {}", DESTINATION_ADDR);
    
    // Spawn a dedicated thread to accept destination clients
    // This runs concurrently with the source client listener
    thread::spawn(move || {
        for stream in dest_listener.incoming() {
            match stream {
                Ok(stream) => {
                    let mut address = String::from("unknown");
                    if let Ok(addr) = stream.peer_addr() {
                        address = addr.to_string(); 
                    }
                    let mut dests = destinations_clone.lock().unwrap();
                    if dests.len() >= MAX_DESTINATIONS {
                        println!("Max destination clients reached. Rejecting new connection from {}.", address);
                    } else {
                        println!("New destination client connected: {}", address);
                        dests.push(stream);
                    }
                }
                Err(e) => eprintln!("Error accepting destination client: {}", e),
            }
        }
    });

    // -------------- Source listener thread (just the main thread)  -----------------
    let source_listener = TcpListener::bind(SOURCE_ADDR)?;
    println!("Listening for single source client on {}", SOURCE_ADDR);

    // Loop to handle one source client at a time
    // When a source disconnects, we wait for the next one
    loop {
        match source_listener.accept() {
            Ok((stream, addr)) => {
                println!("Source client connected from: {}", addr);
                // Handle single source client in the main thread
                // This blocks until the client disconnects
                handle_source_client(stream, Arc::clone(&destinations));
                println!("Source client {} disconnected. Waiting for next source client...", addr);
            }
            Err(e) => {
                eprintln!("Error accepting source client: {}", e);
                // Continue listening for the next connection
                continue;
            }
        }
    }
}


// Handles a source client connection
fn handle_source_client(mut stream: TcpStream, destinations: Arc<Mutex<Vec<TcpStream>>>) {
    // We must handle case where the address is not found properly as peer_addr() returns result
    let mut address = String::from("unknown");
    if let Ok(addr) = stream.peer_addr() {
        address = addr.to_string(); 
    }
    // Similarly here with set_read_timeout()
    let timeout = Some(Duration::from_secs(READ_TIMEOUT_SECS));
    if let Err(e) = stream.set_read_timeout(timeout){
        eprintln!("Warning: Could not set read timeout for source {}: {}. Closing connection." , address, e);
        return; 
    }
    
    println!("Now handling messages from source client: {}", address);
    
    // Loop and read messages from the source client
    loop {
        let mut header_buf = [0u8; HEADER_SIZE];
        match stream.read_exact(&mut header_buf) {
            Ok(_) => {
                let header = Header::from_bytes(&header_buf);
                // Validate ctmp header (magic byte and padding fields)
                if !header.is_valid() {
                    eprintln!("Invalid CTMP header from source {}. Disconnecting.", address);
                    break;
                }

                let payload_len = header.payload_length();
                let mut payload_buf = vec![0; payload_len];

                // Read payload into buffer
                if let Err(e) = stream.read_exact(&mut payload_buf) {
                    eprintln!("Failed to read payload of size {} from source {}: {}. Disconnecting.", payload_len, address, e);
                    break;
                }

                // Combine header and payload into final message to send
                let full_message = [header_buf.as_slice(), &payload_buf].concat();
                // Lock destination list whilst we broadcast
                let mut dests = destinations.lock().unwrap();
                println!("Relaying CTMP message of {} bytes from {} to {} destination clients.", full_message.len(), address, dests.len());

                // Iterate through destination clients and remove disconnected clients
                // retain_mut keeps clients that successfully receive the message
                dests.retain_mut(|dest_stream| {
                        // A write failure indicates client has disconnected
                        match dest_stream.write_all(&full_message) {
                            Ok(_) => true,  // Keep this client
                            Err(_) => {
                                println!("Destination client disconnected during broadcast, removing from list.");
                                false  // Remove this client
                            }
                        }
                    });
            }
            // This error means the client closed the connection gracefully
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                println!("Source client {} disconnected gracefully.", address);
                break;
            }
            // This will catch other errors, such as the read timeout
            Err(e) => {
                eprintln!("Error reading from source {}: {}. Disconnecting.", address, e);
                break;
            }
        }
    }
}