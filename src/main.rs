

#[macro_use] extern crate anyhow;
#[macro_use] extern crate log;
#[macro_use] extern crate clap;

mod opt;
mod tcp;
mod ssl;

fn main() {
    env_logger::init();
    let opt = opt::load();
    match opt {
        opt::Opt{tcp_port, ..} if tcp_port != 0 => {
            if let anyhow::Result::Err(error) = async_std::task::block_on(tcp::run_tcp(opt)) {
                error!("{}", error);
            }
        }
        opt::Opt{ssl_port, ..} if ssl_port != 0 => {
            if let anyhow::Result::Err(error) = async_std::task::block_on(ssl::run_ssl(opt)) {
                error!("{}", error);
            }
        }
        _ => {
            eprintln!("TCP or SSL Listen Port not Set.");
        }
    }
}
