use std::io;
use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;

const NUL: u8 = 0;
const OP_RRQ: u8 = 1;
const OP_WRQ: u8 = 2;
const OP_DATA: u8 = 3;
const OP_ACK: u8 = 4;
const OP_ERROR: u8 = 5;
const MAX_RETRY: i32 = 5;
const TIMEOUT: Option<Duration> = Some(Duration::new(5, 0));

pub fn get(dst: Ipv4Addr, file: String, dport: u16, mode: String) -> io::Result<()> {
    let mut recv_buf = [0u8; 1024];
    let mut rrq_buf = vec![NUL, OP_RRQ];
    rrq_buf.extend(file.as_bytes());
    rrq_buf.push(NUL);
    rrq_buf.extend(mode.as_bytes());
    rrq_buf.push(NUL);
    let rrq_buf = rrq_buf.as_slice();

    let socket = UdpSocket::bind("127.0.0.1:0").expect("Ephemeral port is not available");
    socket.set_read_timeout(TIMEOUT).expect("set_read_timeout call failed");
    socket.set_write_timeout(TIMEOUT).expect("set_write_timeout call failed");
    socket.send_to(rrq_buf, format!("{}:{}", dst, dport))?;

    match socket.recv_from(&mut recv_buf) {
        Ok((number_of_bytes, src_addr)) => {
            let filled_buf = &recv_buf[..number_of_bytes];
            println!("{:?}", src_addr);
            println!("{:?}", filled_buf);
        },
        Err(e) => {
            println!("Failed to receive the first DATA packet: {:?}", e);
            return Err(e)
        }
    }

    Ok(())
}





pub fn put(dst: Ipv4Addr, file: String, dport: u16, mode: String) {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("Ephemeral port is not available");
    socket.set_read_timeout(TIMEOUT).expect("set_read_timeout call failed");
    socket.set_write_timeout(TIMEOUT).expect("set_write_timeout call failed");
    socket.connect(format!("{}:{}", dst, dport)).expect("connect function failed");

}