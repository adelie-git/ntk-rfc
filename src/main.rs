use clap::{Parser, ValueEnum};
mod tftp;
mod ftp;
mod syslog;
mod smtp;

#[derive(Parser, Debug)]
struct Cli {
    /// Select only protocols within
    protocol: Protocol
}

#[derive(Debug, Clone, ValueEnum)]
enum Protocol {
    Tftp,
    Ftp,
    Syslog,
    Smtp
}

impl Cli {
    fn run(self) {
        use Protocol::*;
        match self.protocol {
            Tftp => {
                tftp::tftpd::run();
            },
            Ftp => {
                ftp::ftpd::run();
            },
            Syslog => {
                syslog::syslogd::run();
            },
            Smtp => {
                smtp::smtpc::run();
                //smtp::smtpd::run();
            }
        }
    }
}

fn main() {
    Cli::parse().run();
}
