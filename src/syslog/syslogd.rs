use std::net::UdpSocket;

#[derive(Debug)]
struct Syslog {
    header: SyslogHeader,
    data: SyslogStructuredData,
    msg: Option<SyslogMessage>
}

#[derive(Debug)]
struct SyslogHeader {
    len: Option<u16>,
    pri: String,
    prival: u8,
    facility: String,
    severity: String,
    version: u8,
    timestamp: String,
    hostname: String,
    appname: String,
    procid: String,
    msgid: String
}

#[derive(Debug)]
struct SyslogStructuredData {
    data: String
}

#[derive(Debug)]
struct SyslogMessage {
    msg: String,
    msg_utf8: String
}

pub fn run() {
    // Listen 514/udp
    let socket = UdpSocket::bind("127.0.0.1:514").expect("Binding to 127.0.0.1:514 failed.");
    // Accept 2048 bytes
    let mut accept_buf = [0; 2048];

    // Server loop
    loop {
        match socket.recv_from(&mut accept_buf) {
            Ok((byte_size, _)) => {
                // Receive buffer
                let recv_buf = &mut accept_buf[..byte_size];
                // Index of '['
                let index = match recv_buf.iter().position(|x| x == &91u8) {
                    Some(v) => v,
                    None => recv_buf.len() - 2usize
                };
                // Split into SyslogHeader and SyslogStructuredData_SyslogMessage
                let (header, data_and_msg) = recv_buf.split_at(index.into());
                // Index of pairs ']' and ' '
                let index = match data_and_msg.windows(2).position(|x| x[0] == 93u8 && x[1] == 32u8) {
                    Some(v) => v + 1usize,
                    None => data_and_msg.len()
                };
                // Split into SyslogStructuredData and SyslogMessage
                let (data, message) = data_and_msg.split_at(index.into());
                // Parse SyslogHeader
                let syslog_header = parse_header(header);
                // Parse SyslogStructuredData
                let syslog_data = parse_data(data);
                // Parse SyslogMessage
                let syslog_message = parse_message(message);
                // Syslog
                let syslog = Syslog {
                    header: syslog_header,
                    data: syslog_data,
                    msg: syslog_message
                };
                println!("Syslog: {:?}", syslog);
            },
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
        }
    }
}

fn parse_header(header: &[u8]) -> SyslogHeader {
    // header iterator
    let mut iter = header.split(|x| x == &32u8);
    // length
    let option_len: Option<u16> = if header[0] != 60u8 {
        let a = iter.next().unwrap();
        let b = String::from_utf8(a.to_vec()).unwrap();
        let c = b.parse::<u16>().unwrap();
        Some(c)
    } else {
        None
    };
    // pri, prival
    let pri_and_version = iter.next().unwrap();
    let pri_ary = &pri_and_version[..pri_and_version.len()-1];
    let pri = String::from_utf8(pri_ary.to_vec()).unwrap();
    let prival_ary = &pri_ary[1..pri_ary.len()-1];
    let prival = String::from_utf8(prival_ary.to_vec()).unwrap();
    let prival = prival.parse::<u8>().unwrap();
    // facility, severity
    let (facility, severity) = get_facility_and_severity(prival);
    // version
    let version = *pri_and_version.last().unwrap();
    // timestamp
    let timestamp_ary = iter.next().unwrap();
    let timestamp = String::from_utf8(timestamp_ary.to_vec()).unwrap();
    // hostname
    let hostname = if let Some(v) = iter.next() {
        String::from_utf8(v.to_vec()).unwrap()
    } else {
        "-".to_string()
    };
    // appname
    let appname = if let Some(v) = iter.next() {
        String::from_utf8(v.to_vec()).unwrap()
    } else {
        "-".to_string()
    };
    // procid
    let procid = if let Some(v) = iter.next() {
        String::from_utf8(v.to_vec()).unwrap()
    } else {
        "-".to_string()
    };
    // msgid
    let msgid = if let Some(v) = iter.next() {
        String::from_utf8(v.to_vec()).unwrap()
    } else {
        "-".to_string()
    };
    // syslog header
    let syslog_header =  SyslogHeader {
        len: option_len,
        pri: pri,
        prival: prival,
        facility: facility,
        severity: severity,
        version: version,
        timestamp: timestamp,
        hostname: hostname,
        appname: appname,
        procid: procid,
        msgid: msgid
    };

    // resturn syslog header
    // println!("syslog_header: {:?}", syslog_header);
    syslog_header
}

fn get_facility_and_severity(prival: u8) -> (String, String) {
    let mut f_code = 0;
    let mut s_code = 0;

    // calc facility and severity
    'outer: for x in 0..24 {
        for y in 0..8 {
            if x * 8 + y == prival {
                f_code = x;
                s_code = y;
                break 'outer;
            }
        }
    };

    // facility
    let facility = match f_code {
        0 => "kern",
        1 => "user",
        2 => "mail",
        3 => "daemon",
        4 => "auth",
        5 => "syslog",
        6 => "lpr",
        7 => "news",
        8 => "uucp",
        9 => "cron",
        10 => "authpriv",
        11 => "ftp",
        12 => "ntp",
        13 => "logaudit",
        14 => "logalert",
        15 => "clock",
        16 => "local0",
        17 => "local1",
        18 => "local2",
        19 => "local3",
        20 => "local4",
        21 => "local5",
        22 => "local6",
        23 => "local7",
        _ => panic!("Not facility code.")
    };

    // severity
    let severity = match s_code {
        0 => "emerg",
        1 => "alert",
        2 => "crit",
        3 => "err",
        4 => "warning",
        5 => "notice",
        6 => "info",
        7 => "debug",
        _ => panic!("Not severity code.")
    };

    (facility.to_string(), severity.to_string())
}

fn parse_data(data: &[u8]) -> SyslogStructuredData {
    // trim
    let structured_data = match String::from_utf8(data.to_vec()) {
        Ok(v) => v.trim().to_string(),
        Err(_) => "-".to_string()
    };
    // syslog structured data
    let syslog_data = SyslogStructuredData {
        data: structured_data
    };
    // resturn syslog structured data
    // println!("syslog_data: {:?}", syslog_data);
    syslog_data
}

fn parse_message(message: &[u8]) -> Option<SyslogMessage> {
    // ignore if no meesage or no BOM 
    if message.len() == 0usize || message[0..4] != [32u8, 66u8, 79u8, 77u8] {
        return None
    }

    // raw message
    let raw_msg = match String::from_utf8(message.to_vec()) {
        Ok(v) => v,
        Err(_) => return None
    };
    // trim BOM message
    let msg_utf8 = match String::from_utf8(message[4..].to_vec()) {
        Ok(v) => v.trim().to_string(),
        Err(_) => return None
    };
    // syslog message
    let syslog_message = SyslogMessage {
        msg: raw_msg,
        msg_utf8: msg_utf8
    };

    // resturn syslog message
    // println!("syslog_message: {:?}", syslog_message);
    Some(syslog_message)
}