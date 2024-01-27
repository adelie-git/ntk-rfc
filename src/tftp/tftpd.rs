use std::borrow::Borrow;
use std::io;
use std::io::{Read, Write};
use std::net::{UdpSocket, SocketAddr};
use std::str;
use std::path::PathBuf;
use std::fs::{File, create_dir};
use std::time::Duration;
use dirs;
use simplelog::*;
use log::{self, LevelFilter};

const NUL: u8 = 0;
const OP_RRQ: u8 = 1;
const OP_WRQ: u8 = 2;
const OP_DATA: u8 = 3;
const OP_ACK: u8 = 4;
const OP_ERROR: u8 = 5;
const MAX_RETRY: i32 = 5;
const TIMEOUT: Option<Duration> = Some(Duration::new(5, 0));

/// Support only RFC1350
pub fn run() {
    let tftp_root = dirs::desktop_dir().unwrap().join("tftp-root");
    let logfile = tftp_root.join("tftp_server.log");
    let socket = UdpSocket::bind("127.0.0.1:69").expect("Binding to 127.0.0.1:69 failed.");

    if !tftp_root.exists() {
        create_dir(&tftp_root).expect("Could not create directory.");
    }

    let _ = CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), File::create(logfile).unwrap())
            ]
    );

    loop {
        // Client's first request packet.
        // Long file names are not acceptable.
        let mut accept_buf = [0; 40];

        match socket.recv_from(&mut accept_buf) {
            Ok((byte_size, src_addr)) => {
                let recv_buf = &mut accept_buf[..byte_size];
                let opcode = &recv_buf[1];

                // The first byte must be 0x00.
                // The second byte is opcode, and the first opcode must be 0x01 or 0x02.
                if &recv_buf[0] != &NUL && (opcode != &OP_RRQ || opcode != &OP_WRQ) {
                    log::debug!("Receving invalid packet: {:?}", &recv_buf);
                    log::debug!("Ignore this packet and wait again.");
                    continue
                }

                let iter = &mut recv_buf[2..recv_buf.len()-1].split(|num| num == &NUL);
                let citer = &mut recv_buf[2..recv_buf.len()-1].split(|num| num == &NUL);
                if citer.count() != 2usize {
                    log::debug!("Receving invalid packet: {:?}", &recv_buf);
                    log::debug!("Ignore this packet and wait again.");
                    continue
                }

                let filename = str::from_utf8(iter.next().unwrap()).unwrap();
                let path = tftp_root.join(filename);
                log::debug!("filename: {:?}", filename);

                let mode = str::from_utf8(iter.next().unwrap()).unwrap().to_lowercase();
                let mode = mode.as_str();
                log::debug!("mode: {:?}", mode);

                match mode {
                    "netascii" | "octet" => (),
                    "mail" => {
                        // Mail mode is not available.
                        let err_buf = build_err_packet(4u8, "Mail mode is not available.");
                        if let Err(e) = socket.send_to(&err_buf, src_addr) {
                            log::error!("Failed to send: {:?}", e);
                        }
                        log::debug!("Receving require mail mode packet: {:?}", &recv_buf);
                        log::debug!("Send error packet and wait again.");
                        continue
                    }
                    _ => {
                        // Expect netascii, octet and mail. 
                        let err_buf = build_err_packet(4u8, "Invalid mode.");
                        if let Err(e) = socket.send_to(&err_buf, src_addr) {
                            log::error!("Failed to send: {:?}", e);
                        }
                        log::debug!("Receving require invalid mode packet: {:?}", &recv_buf);
                        log::debug!("Send error packet and wait again.");
                        continue
                    }
                }
                
                match opcode {
                    &OP_RRQ => {
                        if !path.exists() || !path.is_file() {
                            let err_buf = build_err_packet(1u8, "Request file not found.");
                            if let Err(e) = socket.send_to(&err_buf, src_addr) {
                                log::error!("[RRQ]Failed to send: {:?}", e);
                            }
                            log::debug!("[RRQ]Receving require non-existing file packet: {:?}", &recv_buf);
                            log::debug!("[RRQ]Send error packet and wait again.");
                            continue
                        }
                        if let Err(e) = rrq_packet(src_addr, path, mode) {
                            log::error!("[RRQ]Failed to process:{:?}", e);
                        };
                    },
                    &OP_WRQ => {
                        if path.exists() {
                            let err_buf = build_err_packet(6u8, "Request file already existed.");
                            if let Err(e) = socket.send_to(&err_buf, src_addr) {
                                log::error!("[WRQ]Failed to send: {:?}", e);
                            }
                            log::debug!("[WRQ]Receving require existing file packet: {:?}", &recv_buf);
                            log::debug!("[WRQ]Send error packet and wait again.");
                            continue
                        }
                        if let Err(e) = wrq_packet(src_addr, path) {
                            log::error!("[WRQ]Failed to process:{:?}", e);
                        };
                    },
                    _ => {
                        log::error!("Unexpected error.");
                        panic!("Coming here means a probably coding miss.")
                    }
                }
            },
            Err(e) => {
                log::error!("Couldn't recieve request: {:?}", e);
            }
        }
    }
}

fn rrq_packet(client_addr: SocketAddr, path: PathBuf, mode: &str) -> io::Result<()> {
    let mut file_buf = Vec::new();
    let mut data_packet = Vec::new();
    log::info!("[RRQ]Process start.");
    
    match mode {
        // Convert to 0x64(@) if it is not ascii character.
        // Also, if 0x13(CR) is not followed by 0x10(LF) or 0x00(NUL), add 0x10(LF).
        "netascii" => {
            let mut buf = Vec::new();
            File::open(&path)?.read_to_end(&mut buf)?;
            let mut conv_buf = buf.into_iter().map(|x| if x.is_ascii() { x } else { 64u8 }).peekable();
            while let Some(v) = conv_buf.next() {
                file_buf.push(v);
                if v == 13u8 {
                    if let Some(vv) = conv_buf.peek().copied() {
                        if vv != 10u8 && vv !=0u8 {
                            file_buf.push(10u8);
                        }
                    }
                }
            }
        },
        // Simply read a sequence of bytes.
        "octet" => {
            File::open(&path)?.read_to_end(&mut file_buf)?;
        },
        _ => {
            log::error!("Unexpected error.");
            panic!("Coming here means a probably coding miss.")
        }
    }
    
    let mut buf_iter = file_buf.chunks(512usize).collect::<Vec<&[u8]>>().clone().into_iter().enumerate();

    loop {
        match buf_iter.next() {
            Some((mut i, v)) => {
                i = i + 1;
                let mut packet = vec![NUL , OP_DATA];
                packet.extend((i as u16).to_be_bytes());
                packet.extend(v);
                data_packet.push(packet)
            },
            None => {
                let buf_size = file_buf.len();
                if buf_size % 512 == 0 {
                    let mut packet = vec![NUL , OP_DATA];
                    let block = (buf_size / 512 + 1) as u16; 
                    packet.extend(block.to_be_bytes());
                    packet.push(0u8);
                    data_packet.push(packet)
                }
                break
            }
        }
    }

    let mut data_packet_iter = data_packet.clone().into_iter();
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.connect(client_addr)?;

    loop {
        match data_packet_iter.next() {
            Some(packet) => {
                let mut retry_count = 0;
                let mut buf = [0; 4];
                while retry_count < MAX_RETRY {
                    match socket.send(&packet) {
                        Ok(byte_size) => {
                            log::debug!("byte: {:?}", byte_size)
                        },
                        Err(e) => {
                            log::error!("SendError: {:?}", e);
                            retry_count += 1;
                            continue;
                        }
                    };

                    match socket.set_read_timeout(TIMEOUT) {
                        Ok(_) => {
                            match socket.recv(&mut buf) {
                                Ok(byte_size) => {
                                    let recv_packet = &buf[..byte_size];
                                    log::debug!("received {byte_size} bytes {:?}", recv_packet);
                                    if recv_packet[1] == OP_ACK && recv_packet[2..4] == packet[2..4] {
                                        break;
                                    }
                                    retry_count += 1;
                                    continue;
                                },
                                Err(e) => {
                                    log::debug!("recv function failed: {:?}", e);
                                    retry_count += 1;
                                    continue;
                                }
                            }
                        },
                        Err(_) => {
                            retry_count += 1;
                            continue;
                        }
                    }
                }
                if retry_count >= MAX_RETRY {
                    return Err(io::Error::new(io::ErrorKind::NotConnected,
                         "The maximum number of retries has been reached."))
                }
            },
            None => break
        }
    }
    log::info!("[RRQ]Process completed!");
    Ok(())
}

fn wrq_packet(client_addr: SocketAddr, path: PathBuf) -> io::Result<()> {
    let mut file_buf: Vec<u8> = Vec::new();
    let mut ack_buf = vec![NUL, OP_ACK];
    let mut ack = 1u16;

    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.connect(client_addr)?;

    match socket.send(&[0u8, 4u8, 0u8, 0u8]) {
        Ok(byte_size) => {
            log::debug!("byte: {:?}", byte_size)
        },
        Err(e) => {
            log::error!("SendError: {:?}", e);
        }
    };

    loop {
        let mut retry_count = 0;
        match socket.set_read_timeout(TIMEOUT) {
            Ok(_) => {
                let mut buf = vec![0; 516];
                match socket.recv(&mut buf) {
                    Ok(byte_size) => {
                        let recv_packet = &buf[..byte_size];
                        log::debug!("received {byte_size} bytes {:?}", recv_packet);
                        if recv_packet[1] == OP_DATA && recv_packet[2..4] == ack.to_be_bytes() {
                            ack_buf.extend(ack.to_be_bytes());
                            match socket.send(&ack_buf) {
                                Ok(_) => {
                                    if recv_packet.len() == 5 && recv_packet[4] == 0u8 {
                                        break;
                                    }
                                    file_buf.extend(&recv_packet[4..]);
                                    if recv_packet[4..].len() < 512 {
                                        break;
                                    }
                                    ack += 1;
                                },
                                Err(_) => {
                                    retry_count += 1;
                                }
                            }
                        } else {
                            retry_count += 1;
                        }
                    },
                    Err(e) => {
                        log::error!("recv function failed: {e:?}");
                        retry_count += 1;
                    }
                }
            },
            Err(_) => {
                log::error!("recv timeout");
                retry_count += 1;
            }
        };
        if retry_count >= MAX_RETRY {
            return Err(io::Error::new(io::ErrorKind::NotConnected,
                    "The maximum number of retries has been reached."))
        }
    }
    File::create(path)?.write_all(&file_buf)?;
    println!("[WRQ]Process completed!");
    Ok(())
}

fn build_err_packet(code: u8, msg: &str) -> Vec<u8> {
    let mut packet = vec![NUL, OP_ERROR, NUL];
    packet.push(code);
    packet.extend(msg.as_bytes());
    packet.push(NUL);
    packet
}
