use clap::{Parser, Subcommand};
use std::net::Ipv4Addr;
mod tftp;
mod ftp;
mod syslog;
mod smtp;

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands
    // /// Select only protocols within
    // protocol: Protocol
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(subcommand)]
    Tftp(TftpSub),
    #[command()]
    Ftp,
    #[command()]
    Syslog,
    #[command()]
    Smtp,
}

#[derive(Subcommand, Debug)]
enum TftpSub {
    #[command()]
    Listen,
    #[command()]
    Get {
        /// destination address
        #[arg()]
        dst: Ipv4Addr,
        /// target file
        #[arg()]
        file: String,
        /// destination port
        #[arg(default_value_t = 69)]
        dport: u16,
        /// transfer mode
        #[arg(default_value = "octet")]
        mode: String
    },
    #[command()]
    Put {
        /// destination address
        #[arg()]
        dst: Ipv4Addr,
        /// target file
        #[arg()]
        file: String,
        /// destination port
        #[arg(short, default_value_t = 69)]
        dport: u16,
        /// transfer mode
        #[arg(short, default_value = "octet")]
        mode: String
    }
}

impl Commands {
    fn run(self) {
        use Commands::*;
        match self {
            Tftp(sub) => {
                use TftpSub::*;
                match sub {
                    Listen => {
                        tftp::tftpd::run()
                    },
                    Get { dst, file, dport, mode } => {
                        if let Err(e) = tftp::tftpc::get(dst, file, dport, mode) {
                            println!("{:?}", e);
                        }
                    },
                    Put { dst, file, dport, mode } => {
                        tftp::tftpc::put(dst, file, dport, mode)
                    },
                }
            },
            Ftp => {
                ftp::ftpd::run();
            },
            Syslog => {
                syslog::syslogd::run();
            },
            Smtp => {
                smtp::smtpc::run();
            }
        }
    }
}

fn main() {
    Cli::parse().command.run();
}
