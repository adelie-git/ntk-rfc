use clap::{Parser, ValueEnum};
mod tftpd;
mod ftpd;
mod syslogd;

#[derive(Parser, Debug)]
struct Cli {
    /// Select only protocols within
    protocol: Protocol
}

#[derive(Debug, Clone, ValueEnum)]
enum Protocol {
    Tftp,
    Ftp,
    Syslog
}

impl Cli {
    fn run(self) {
        use Protocol::*;
        match self.protocol {
            Tftp => {
                tftpd::run();
            },
            Ftp => {
                ftpd::run();
            },
            Syslog => {
                syslogd::run();
            }
        }
    }
}

fn main() {
    Cli::parse().run();
}
