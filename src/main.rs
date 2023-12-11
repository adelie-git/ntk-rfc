use clap::{Parser, ValueEnum};
mod tftpd;

#[derive(Parser, Debug)]
struct Cli {
    /// Select only protocols within
    protocol: Protocol
}

#[derive(Debug, Clone, ValueEnum)]
enum Protocol {
    Tftp
}

impl Cli {
    fn run(self) {
        use Protocol::*;
        match self.protocol {
            Tftp => {
                tftpd::run();
            }
        }
    }
}

fn main() {
    Cli::parse().run();
}
