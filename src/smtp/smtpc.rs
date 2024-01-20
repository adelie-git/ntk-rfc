use std::{net::TcpStream, io::{Write, Read, self}, time::Duration};
use chrono::Local;
use windows::Win32::Networking::WinSock;
use get_last_error::Win32Error;

pub fn run() {
    let mut buf = [0u8; 512];
    let mut stream = TcpStream::connect("127.0.0.1:25").unwrap();
    // Default write timeout 3 seconds
    stream.set_write_timeout(Some(Duration::new(3, 0))).unwrap();

    // Initial 220 Message: 5 Minutes
    stream.set_read_timeout(Some(Duration::new(300, 0))).unwrap();
    match stream.read(&mut buf) {
        Ok(u) => {
            let reply = String::from_utf8(buf[..u].to_vec()).unwrap();
            println!("{}", reply);
        },
        Err(e) => {
            println!("{:?}", e);
            return
        }
    }

    let mail_ehlo = format!("EHLO {}\r\n", get_hostname().unwrap());
    write_and_read(&stream, mail_ehlo, 3).unwrap();

    let mail_from = "from@example.com";
    write_and_read(&stream, format!("MAIL FROM:<{}>\r\n", mail_from), 5).unwrap();

    let mail_to = "to@example.com";
    write_and_read(&stream, format!("RCPT TO:<{}>\r\n", mail_to), 5).unwrap();

    let mail_data = String::from("DATA\r\n");
    write_and_read(&stream, mail_data, 2).unwrap();

    let mime = "1.0";
    let date = Local::now();
    let subject = "Test for SMTP";
    let contype = "text/plain; charset=us-ascii";
    let body = "This mail is test.";
    let mail_data_block = format!("\
                                        MIME-Version: {}\r\n\
                                        From: {}\r\n\
                                        To: {}\r\n\
                                        Date: {}\r\n\
                                        Subject: {}\r\n\
                                        Content-Type: {}\r\n\
                                        {}\r\n\
                                        .\r\n",
                                         mime, mail_from, mail_to, date, subject, contype, body);
    write_and_read(&stream, mail_data_block, 10).unwrap();

    let mail_quit = String::from("QUIT\r\n");
    write_and_read(&stream, mail_quit, 3).unwrap();

    stream.shutdown(std::net::Shutdown::Both).unwrap();

}

fn get_hostname() -> Result<String, Win32Error> {
    let mut buf = [0u8; 512];
    let response = unsafe {
        WinSock::gethostname(&mut buf)
    };

    if response != 0 {
        let win32err = Win32Error::get_last_error();
        println!("{:?}", win32err);
        return Err(win32err)
    }

    let name_buf = &buf.into_iter().filter(|x| x != &0u8).collect::<Vec<u8>>();
    Ok(String::from_utf8(name_buf.to_vec()).unwrap())
}

fn write_and_read(mut stream: &TcpStream, message: String, minutes: u64) -> io::Result<()> {
    let mut buf = [0u8; 512];

    println!("{}", message);
    if let Err(e) = stream.write_all(message.as_bytes()) {
        println!("{:?}", e);
        return Err(e)
    }

    stream.set_read_timeout(Some(Duration::new(minutes * 60, 0))).unwrap();

    match stream.read(&mut buf) {
        Ok(u) => {
            let reply = String::from_utf8(buf[..u].to_vec()).unwrap();
            println!("{}", reply);
        },
        Err(e) => {
            println!("{:?}", e);
            return Err(e)
        }
    }

    Ok(())
}