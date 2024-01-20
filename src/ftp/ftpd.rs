use std::net::{TcpListener, TcpStream, Shutdown};
use std::fs::File;
use std::path::PathBuf;
use std::io::{Write, BufRead, BufReader, Read};
use tokio::io::Interest;
use tokio::net::{TcpStream as TokioTcpStream, TcpSocket};

//const FTP_CMD: [&str; 11] = ["USER", "PASS", "QUIT", "PORT", "TYPE", "MODE", "STRU", "RETR", "STOR", "NOOP", "OPTS"];

pub fn run() {
    // Create ftp root directory
    let ftp_root: PathBuf = dirs::desktop_dir().unwrap().join("ftp-root");
    if !ftp_root.exists() {
        std::fs::create_dir(&ftp_root).expect("Could not create directory.");
    }

    // listen ftp connection
    let listener = TcpListener::bind("127.0.0.1:21").unwrap();
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        handle_control_connection(stream, ftp_root);
        break;
    }
}

fn handle_control_connection(mut stream: TcpStream, fs_path: PathBuf) {
    // Server reply -> ok
    stream.write_all(get_reply_message(220)).unwrap();
    // buffer reader
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    // destination address
    let mut dst_addr = String::new();
    // ftp user
    let user = "ftp";
    // ftp user passwd
    let passwd = "ftp";
    // command sequence
    let mut correct_sequence = false;
    // runtime
    let rt = tokio::runtime::Runtime::new().unwrap();

    loop {
        let mut buf = Vec::new();
        if let Err(e) = reader.read_until(10, &mut buf) {
            println!("Buffer reading error: {:?}", e);
            break;
        }

        // whitespace delimited iterator
        let mut iter = buf.split(|b| b == &32u8);
        
        if let Some(v) =  iter.next() {
            // ftp command
            let buf = v.to_ascii_uppercase();
            let buf = buf.as_slice();
            let cmd = std::str::from_utf8(buf).unwrap();
            let cmd = cmd.trim();
            println!("CMD: {}", cmd);

            // ftp value
            let value = if let Some(v) = iter.next() {
                let val = std::str::from_utf8(v).unwrap();
                println!("VAL: {}", val);
                val.trim()
            } else {
                ""
            };
            
            // Processing by ftp command
            match cmd {
                "USER" => {
                    if user == value {
                        correct_sequence = true;
                        stream.write_all(get_reply_message(331)).unwrap()
                    } else {
                        stream.write_all(get_reply_message(530)).unwrap()
                    }
                },
                "PASS" => {
                    if !correct_sequence {
                        stream.write_all(get_reply_message(503)).unwrap();
                        continue;
                    }
                    if passwd == value {
                        stream.write_all(get_reply_message(230)).unwrap()
                    } else {
                        stream.write_all(get_reply_message(530)).unwrap()
                    }
                },
                "PORT" => {
                    let addrs: Vec<&str> = value.split(",").collect();
                    let addr = format!("{}.{}.{}.{}", addrs[0], addrs[1], addrs[2], addrs[3]);
                    let port = addrs[4].parse::<i32>().unwrap() * 256 + addrs[5].parse::<i32>().unwrap();
                    dst_addr = format!("{}:{}", addr, port);
                    println!("{:?}", dst_addr);
                    stream.write_all(get_reply_message(200)).unwrap()
                },
                "RETR" => {
                    stream.write_all(get_reply_message(150)).unwrap();
                    let file = match control_filesystem("read", fs_path.join(value)) {
                        Ok(v) => v,
                        Err(e) => {
                            println!("Error: {:?}", e);
                            continue;
                        }
                    };
                    rt.block_on(handle_w_data_connection(dst_addr.clone(), file));
                    stream.write_all(get_reply_message(226)).unwrap();
                },
                "LIST" => {
                    stream.write_all(get_reply_message(150)).unwrap();
                    let ls = match control_filesystem("ls", fs_path.clone()) {
                        Ok(v) => v,
                        Err(e) => {
                            println!("Error: {:?}", e);
                            continue;
                        }
                    };
                    rt.block_on(handle_w_data_connection(dst_addr.clone(), ls));
                    stream.write_all(get_reply_message(226)).unwrap();
                }
                "TYPE" => {
                    stream.write_all(get_reply_message(200)).unwrap()
                },
                "STOR" => {
                    stream.write_all(get_reply_message(150)).unwrap();
                    if let Err(e) = rt.block_on(handle_r_data_connection(dst_addr.clone(), fs_path.join(value))) {
                        println!("Error: {:?}", e);
                        continue;
                    }
                    stream.write_all(get_reply_message(226)).unwrap();
                },
                // "XPWD" => {
                //     let pwd = fs_path.to_string_lossy().as_bytes().to_vec();
                //     rt.block_on(handle_data_connection("w", dst_addr.clone(), pwd));
                //     stream.write_all(get_reply_message(200)).unwrap();
                //     continue;
                // },
                "OPTS" => {
                    stream.write_all(get_reply_message(504)).unwrap();
                    continue;
                },
                "QUIT" => {
                    stream.shutdown(Shutdown::Both).unwrap();
                    break;
                },
                _ => {
                    stream.write_all(get_reply_message(502)).unwrap();
                    continue;
                }
            }
        }
    }
}

async fn handle_r_data_connection(dst_addr: String, path: PathBuf) -> Result<(), std::io::Error> {
    let src = "127.0.0.1:20".parse().unwrap();
    let dst = dst_addr.parse().unwrap();
    let socket = TcpSocket::new_v4().unwrap();
    socket.bind(src).unwrap();
    let stream: TokioTcpStream = socket.connect(dst).await.unwrap();

    loop {
        let ready = stream.ready(Interest::READABLE).await.unwrap();

        if ready.is_readable() {
            let mut data = vec![0; 1460];
            match stream.try_read(&mut data) {
                Ok(n) => {
                    let read_buf = &mut data[..n];
                    let mut file = File::create(path)?;
                    file.write_all(read_buf)?;
                    return Ok(())
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                },
                Err(e) => {
                    return Err(e)
                }
            }
        }
    }
}

async fn handle_w_data_connection(dst_addr: String, data: Vec<u8>) {
    let src = "127.0.0.1:20".parse().unwrap();
    let dst = dst_addr.parse().unwrap();
    let socket = TcpSocket::new_v4().unwrap();
    socket.bind(src).unwrap();
    let stream: TokioTcpStream = socket.connect(dst).await.unwrap();

    loop {
        let ready = stream.ready(Interest::WRITABLE).await.unwrap();

        if ready.is_writable() {
            match stream.try_write(&data) {
                Ok(_) => {
                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                    break;
                }
            }
        }
    }
}

fn control_filesystem(method: &str, path: PathBuf) -> Result<Vec<u8>, std::io::Error> {
    match method {
        "ls" => {
            let mut buf = Vec::new();
            let mut crlf = vec![13u8, 10u8];
            let entries = std::fs::read_dir(path).unwrap();
            for elm in entries {
                let file = elm.unwrap().file_name();
                let mut buff = file.into_encoded_bytes();
                buf.append(&mut buff);
                buf.append(&mut crlf);
            }
            Ok(buf)
        },
        "read" => {
            let mut file = File::open(path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            Ok(buf)
        },
        _ => {
            panic!("oh...")
        }
    }

}

fn get_reply_message(code: i32) -> &'static [u8] {
    let message = match code {
        110 => "110 Restart marker reply.\r\n",
        120 => "120 Service ready in 86400 minutes.\r\n",
        125 => "125 Data connection already open; transfer starting.\r\n",
        150 => "150 File status okay; about to open data connection.\r\n",
        200 => "200 Command okay.\r\n",
        202 => "202 Command not implemented, superfluous at this site.\r\n",
        211 => "211 System status, or system help reply.\r\n",
        212 => "212 Directory status.\r\n",
        213 => "213 File status.\r\n",
        214 => "214 Help message.\r\n",
        215 => "215 NAME system type.\r\n",
        220 => "220 Service ready for new user.\r\n",
        221 => "221 Service closing control connection.\r\n",
        225 => "225 Data connection open; no transfer in progress.\r\n",
        226 => "226 Closing data connection.\r\n",
        227 => "227 Entering Passive Mode (h1,h2,h3,h4,p1,p2).\r\n",
        230 => "230 User logged in, proceed.\r\n",
        250 => "250 Requested file action okay, completed.\r\n",
        257 => "257 \"PATHNAME\" created.\r\n",
        331 => "331 User name okay, need password.\r\n",
        332 => "332 Need account for login.\r\n",
        350 => "350 Requested file action pending further information.\r\n",
        421 => "421 Service not available, closing control connection.\r\n",
        425 => "425 Can't open data connection.\r\n",
        426 => "426 Connection closed; transfer aborted.\r\n",
        450 => "450 Requested file action not taken.\r\n",
        451 => "451 Requested action aborted: local error in processing.\r\n",
        452 => "452 Requested action not taken.\r\n",
        500 => "500 Syntax error, command unrecognized.\r\n",
        501 => "501 Syntax error in parameters or arguments.\r\n",
        502 => "502 Command not implemented.\r\n",
        503 => "503 Bad sequence of commands.\r\n",
        504 => "504 Command not implemented for that parameter.\r\n",
        530 => "530 Not logged in.\r\n",
        532 => "532 Need account for storing files.\r\n",
        550 => "550 Requested action not taken.\r\n",
        551 => "551 Requested action aborted: page type unknown.\r\n",
        552 => "552 Requested file action aborted.\r\n",
        553 => "553 Requested action not taken.\r\n",
        _ => panic!("oh...")
    };

    message.as_bytes()
}