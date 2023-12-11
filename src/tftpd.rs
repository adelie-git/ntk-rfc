use std::io::{Read, Write};
use std::net::{UdpSocket, SocketAddr};
use std::str;
use std::io;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::fs::create_dir;
use std::time::Duration;
use dirs;
use simplelog::*;
use log::{self, LevelFilter};

struct OpCode {}
struct TftpLimit {}

impl OpCode {
    const NUL: u8 = 0;
    const RRQ: u8 = 1;
    const WRQ: u8 = 2;
    const DATA: u8 = 3;
    const ACK: u8 = 4;
    const ERROR: u8 = 5;
}

impl TftpLimit {
    const RETRY: i32 = 5;
    const TIMEOUT: Option<Duration> = Some(Duration::new(5, 0));
}

/// Support only RFC1350
pub fn run() {
    let tftp_root: PathBuf = dirs::desktop_dir().unwrap().join("tftp-root");
    let logfile = tftp_root.join("tftp_server.log");
    let socket = UdpSocket::bind("127.0.0.1:69").expect("Binding to 127.0.0.1:69 failed.");

    let _ = CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), File::create(logfile).unwrap())
            ]
    );

    if !tftp_root.exists() {
        create_dir(&tftp_root).expect("Could not create directory.");
    }

    loop {
        // Client's first request packet.
        // Long file names are not acceptable.
        let mut accept_buf = [0; 40];

        match socket.recv_from(&mut accept_buf) {
            Ok((byte_size, src_addr)) => {
                let recv_buf = &mut accept_buf[..byte_size];
                let first_byte = &recv_buf[0];
                let opcode = &recv_buf[1];

                // The first byte must be 0x00.
                // The second byte is opcode, and the first opcode must be 0x01 or 0x02.
                if first_byte != &OpCode::NUL && opcode != &OpCode::RRQ && opcode != &OpCode::WRQ {
                    log::debug!("Receving invalid packet: {:?}", &recv_buf);
                    log::debug!("Ignore this packet and wait again.");
                    continue
                }

                let iter = &mut recv_buf[2..].split(|num| num == &0u8);
                let filename = match iter.next() {
                    Some(byte) => {
                        log::debug!("filename: {:?}", str::from_utf8(byte).unwrap());
                        str::from_utf8(byte).unwrap()
                    },
                    None => {
                        log::debug!("Receving invalid packet: {:?}", &recv_buf);
                        log::debug!("Ignore this packet and wait again.");
                        continue                       
                    }
                };
                let mode = match iter.next() {
                    Some(byte) => {
                        log::debug!("mode: {:?}", str::from_utf8(byte).unwrap());
                        str::from_utf8(byte).unwrap().to_lowercase()
                    },
                    None => {
                        log::debug!("Receving invalid packet: {:?}", &recv_buf);
                        log::debug!("Ignore this packet and wait again.");
                        continue
                    }
                };

                // Mail mode is not available.
                if mode == "mail" {
                    let msg = String::from("Mail mode is not available.");
                    let err_buf = build_err_packet(4u8, msg);
                    socket.send_to(&err_buf, src_addr).unwrap();
                    log::debug!("Receving require mail mode packet: {:?}", &recv_buf);
                    log::debug!("Send error packet and wait again.");
                    continue
                }

                let path = tftp_root.join(filename);
                let path = path.as_path();

                match opcode {
                    &OpCode::RRQ => {
                        if !path.exists() || !path.is_file() {
                            let msg = String::from("Request file not found.");
                            let err_buf = build_err_packet(1u8, msg);
                            socket.send_to(&err_buf, src_addr).unwrap();
                            log::debug!("[RRQ]Receving require non-existing file packet: {:?}", &recv_buf);
                            log::debug!("[RRQ]Send error packet and wait again.");
                            continue
                        }
                        if let Err(e) = rrq_packet(src_addr, path, mode) {
                            log::error!("[RRQ]Failed to process:{:?}", e);
                        };
                    },
                    &OpCode::WRQ => {
                        if path.exists() {
                            let msg = String::from("Request file already existed.");
                            let err_buf = build_err_packet(6u8, msg);
                            socket.send_to(&err_buf, src_addr).unwrap();
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

fn rrq_packet(client_addr: SocketAddr, path: &Path, mode: String) -> io::Result<()> {
    let mut file_buf = Vec::new();
    let mut data_packet = Vec::new();

    log::info!("[RRQ]Process start.");
    
    match mode.as_str() {
        // Convert to 0x64(@) if it is not ascii character.
        // Also, if 0x13(CR) is not followed by 0x10(LF) or 0x00(NUL), add 0x10(LF).
        "netascii" => {
            let mut buf = Vec::new();
            File::open(&path)?.read_to_end(&mut buf)?;
            let mut conv_buf = buf.into_iter()
                                                            .map(|x| if x.is_ascii() { x } else { 64u8 })
                                                            .peekable();
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
    
    let mut buf_iter = file_buf.chunks(512usize).collect::<Vec<&[u8]>>()
                                                                .clone().into_iter().enumerate();

    loop {
        match buf_iter.next() {
            Some((mut i, v)) => {
                i = i + 1;
                let mut packet = vec![OpCode::NUL , OpCode::DATA];
                packet.extend((i as u16).to_be_bytes());
                packet.extend(v);
                data_packet.push(packet)
            },
            None => {
                let buf_size = file_buf.len();
                if buf_size % 512 == 0 {
                    let mut packet = vec![OpCode::NUL , OpCode::DATA];
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
                while retry_count < TftpLimit::RETRY {
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

                    match socket.set_read_timeout(TftpLimit::TIMEOUT) {
                        Ok(_) => {
                            match socket.recv(&mut buf) {
                                Ok(byte_size) => {
                                    let recv_packet = &buf[..byte_size];
                                    log::debug!("received {byte_size} bytes {:?}", recv_packet);
                                    if recv_packet[1] == OpCode::ACK && recv_packet[2..4] == packet[2..4] {
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
                if retry_count >= TftpLimit::RETRY {
                    return Err(io::Error::new(io::ErrorKind::NotConnected,
                         "The maximum number of retries has been reached."))
                }
            },
            None => break
        }
    }
    log::info!("[RRQ]Process completed!.");
    Ok(())
}

fn wrq_packet(client_addr: SocketAddr, path: &Path) -> io::Result<()> {
    let mut file_buf: Vec<u8> = Vec::new();
    let mut ack_buf = vec![OpCode::NUL, OpCode::ACK];
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
        match socket.set_read_timeout(TftpLimit::TIMEOUT) {
            Ok(_) => {
                let mut buf = vec![0; 516];
                match socket.recv(&mut buf) {
                    Ok(byte_size) => {
                        let recv_packet = &buf[..byte_size];
                        log::debug!("received {byte_size} bytes {:?}", recv_packet);
                        if recv_packet[1] == OpCode::DATA && recv_packet[2..4] == ack.to_be_bytes() {
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
        if retry_count >= TftpLimit::RETRY {
            return Err(io::Error::new(io::ErrorKind::NotConnected,
                    "The maximum number of retries has been reached."))
        }
    }
    File::create(path)?.write_all(&file_buf)?;
    println!("[WRQ]Process completed!");
    Ok(())
}

fn build_err_packet(code: u8, msg: String) -> Vec<u8> {
    let mut packet = vec![OpCode::NUL, OpCode::ERROR, 0u8];
    packet.push(code);
    packet.extend(msg.into_bytes());
    packet.push(0u8);
    packet
}
