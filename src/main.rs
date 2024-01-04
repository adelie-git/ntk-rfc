use clap::{Parser, ValueEnum};
mod tftpd;
mod ftpd;

#[derive(Parser, Debug)]
struct Cli {
    /// Select only protocols within
    protocol: Protocol
}

#[derive(Debug, Clone, ValueEnum)]
enum Protocol {
    Tftp,
    Ftp
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
            }
        }
    }
}

fn main() {
    Cli::parse().run();
}
