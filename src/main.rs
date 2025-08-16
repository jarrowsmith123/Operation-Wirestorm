use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SOURCE_ADDR: &str = "127.0.0.1:33333";
const DESTINATION_ADDR: &str = "127.0.0.1:44444";
const MAGIC: u8 = 0xCC;
const HEADER_SIZE: usize = 8;
const MAX_DESTINATIONS: usize = 100; // Prevents number of destination clients potentially overwhelming server
const READ_TIMEOUT_SECS: u64 = 10; // Prevent slow clients holding connection indefinitely

// ============= CTMP Header structure ============

/// Represents a parsed CTMP message header
/// 1 magic byte = 0xCC
/// 1 byte for the options where bit 1 represents a sensitive message i.e. 0100 0000
/// 2 bytes for the length of payload (16-bit limit provides implicit max payload size of 65,535 bytes)
/// 2 bytes for the checksum
/// 2 bytes of 0s for padding
struct Header {
    magic: u8,
    options: u8,
    length: u16,
    checksum: u16,
    padding: u16,
}

impl Header {
    // Creates a Header struct from an 8-byte buffer
    pub fn from_bytes(bytes: &[u8; HEADER_SIZE]) -> Self {
        let length: u16 = u16::from_be_bytes([bytes[2], bytes[3]]);
        let checksum: u16 = u16::from_be_bytes([bytes[4], bytes[5]]);
        let padding: u16 = u16::from_be_bytes([bytes[6], bytes[7]]);

        Header {
            magic: bytes[0],
            options: bytes[1],
            length,
            checksum,
            padding,
        }
    }

    /// Validates the header fields against CTMP spec
    /// Magic byte must be 0xCC and both padding bytes should be filled with 0s
    /// Length is implicitly bounded by size of u16
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC && self.padding == 0
    }

    pub fn is_sensitive(&self) -> bool {
        (self.options & 0b0100_0000) != 0
    }

    /// Returns the payload length as a usize for vec allocation
    pub fn payload_length(&self) -> usize {
        self.length as usize
    }

    // The checksum is calculated by summing all 16 bit words of the entire message
    // with 0xCCCC as the checksum for calculation
    // We keep adding the sum until it becomes a 16 bit number
    // The checksum is then the ones complement of this number
    // ---- The specification wording is slightly unclear on this but this is my interpretation ------
    pub fn validate_checksum(&self, data: &[u8]) -> bool {
        let mut sum: u32 = 0;
        let mut chunks = data.chunks_exact(2);

        sum += u16::from_be_bytes([self.magic, self.options]) as u32;
        sum += self.length as u32;
        sum += 0xCCCC_u32;

        // Sum all 16-bit words
        for chunk in chunks.by_ref() {
            let word = u16::from_be_bytes([chunk[0], chunk[1]]);
            sum += u32::from(word);
        }

        // If there's an odd byte left, pad it with a zero byte and add to sum
        if let Some(&last_byte) = chunks.remainder().first() {
            let word = u16::from_be_bytes([last_byte, 0]);
            sum += u32::from(word);
        }

        // Fold the 32-bit sum into 16 bits
        while (sum >> 16) > 0 {
            sum = (sum >> 16) + (sum & 0xFFFF);
        }

        let checksum = !sum as u16;

        checksum == self.checksum
    }
}


// ============== Main Server Logic ================

fn main() -> io::Result<()> {
    println!("Starting CTMP Proxy Server...");

    // This list of destination clients must be shared across multiple threads
    // so we store in a Vec and wrap it in an Arc Mutex
    let destinations: Arc<Mutex<Vec<TcpStream>>> =
        Arc::new(Mutex::new(Vec::with_capacity(MAX_DESTINATIONS)));

    // ------------- Destination listener threads  ------------------
    let destinations_clone = Arc::clone(&destinations);
    let dest_listener = TcpListener::bind(DESTINATION_ADDR)?;
    println!("Listening for destination clients on {DESTINATION_ADDR}");

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
                        println!(
                            "Max destination clients reached. Rejecting new connection from {address}."
                        );
                    } else {
                        println!("New destination client connected: {address}");
                        dests.push(stream);
                    }
                }
                Err(e) => eprintln!("Error accepting destination client: {e}"),
            }
        }
    });

    // -------------- Source listener thread   -----------------
    let source_listener = TcpListener::bind(SOURCE_ADDR)?;
    println!("Listening for single source client on {SOURCE_ADDR}");

    // Loop to handle one source client at a time
    // When a source disconnects, we wait for the next one
    loop {
        match source_listener.accept() {
            Ok((stream, addr)) => {
                println!("Source client connected from: {addr}");
                // Handle single source client in the main thread
                // This blocks until the client disconnects
                handle_source_client(stream, Arc::clone(&destinations));
                println!("Source client {addr} disconnected. Waiting for next source client...");
            }
            Err(e) => {
                eprintln!("Error accepting source client: {e}");
                // Continue listening for the next connection
                continue;
            }
        }
    }
}

// -------------- Handle source ----------------

// Handles a source client connection
fn handle_source_client(mut stream: TcpStream, destinations: Arc<Mutex<Vec<TcpStream>>>) {
    // We must handle case where the address is not found properly as peer_addr() returns result
    let mut address = String::from("unknown");
    if let Ok(addr) = stream.peer_addr() {
        address = addr.to_string();
    }
    // Similarly here with set_read_timeout()
    let timeout = Some(Duration::from_secs(READ_TIMEOUT_SECS));
    if let Err(e) = stream.set_read_timeout(timeout) {
        eprintln!(
            "Warning: Could not set read timeout for source {address}: {e}. Closing connection."
        );
        return;
    }

    println!("Now handling messages from source client: {address}");

    // Loop and read messages from the source client
    loop {
        let mut header_buf = [0u8; HEADER_SIZE];
        match stream.read_exact(&mut header_buf) {
            Ok(_) => {
                let header = Header::from_bytes(&header_buf);
                // Validate ctmp header
                // Note that we could continue to keep this source open for more messages
                // but I think it makes sense to just break the connection when considered faulty
                // or if the specification says otherwise
                if !header.is_valid() {
                    eprintln!("Invalid CTMP header from source {address}. Disconnecting.");
                    break;
                }

                let payload_len = header.payload_length();
                let mut payload_buf = vec![0; payload_len];

                // Read payload into buffer
                if let Err(e) = stream.read_exact(&mut payload_buf) {
                    eprintln!(
                        "Failed to read payload of size {payload_len} from source {address}: {e}. Disconnecting."
                    );
                    break;
                }

                // Create full message from header and payload
                let full_message = [header_buf.as_slice(), &payload_buf].concat();

                // For sensitive messages, validate the checksum before forwarding
                if header.is_sensitive() && !header.validate_checksum(&payload_buf) {
                    eprintln!(
                        "Invalid checksum for sensitive message from {address}. Dropping message."
                    );
                    continue; // Drop invalid message, wait for the next one
                }

                let mut dests = destinations.lock().unwrap();
                println!(
                    "Relaying CTMP message of {} bytes from {} to {} destination clients.",
                    full_message.len(),
                    address,
                    dests.len()
                );

                // Iterate through destination clients and remove disconnected clients
                dests.retain_mut(|dest_stream| {
                    match dest_stream.write_all(&full_message) {
                        Ok(_) => true, // Keep this client
                        Err(_) => {
                            println!("Destination client disconnected during broadcast, removing from list.");
                            false // Remove this client
                        }
                    }
                });
            }
            Err(e) => {
                match e.kind() {
                    // TimedOut occurs if the client sends no data for READ_TIMEOUT_SECS after sending a message
                    // It would be easy to remove this timeout as it is not in the specification
                    // I just wanted to consider it
                    io::ErrorKind::WouldBlock => {
                        eprintln!("Source client {address} timed out waiting for data. Disconnecting.");
                    }
                    // This error means the client closed the connection gracefully
                    io::ErrorKind::UnexpectedEof => {
                        println!("Source client {address} disconnected gracefully.");
                    }
                    // Handle all other potential I/O errors.
                    _ => {
                        eprintln!("Error reading message from source {address}: {e}. Disconnecting.");
                    }
                }
                break;
            }

        }
    }
}
