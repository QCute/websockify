//! TCP with SSL Proxy

#![allow(irrefutable_let_patterns)]
use futures_util::io::AsyncReadExt;
use futures_util::io::AsyncWriteExt;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use async_std::prelude::FutureExt;
use crate::opt::Opt;

// ssl server entry
pub async fn run_ssl(opt: Opt) -> anyhow::Result<()> {
    // server
    let socket = async_std::net::TcpListener::bind(format!("{}:{}", opt.listen, opt.ssl_port)).await?;
    debug!("Bind: {}:{}", opt.listen, opt.tcp_port);
    // acceptor stream
    let acceptor = load_tls_acceptor(&opt)?;
    while let (stream, address) = socket.accept().await? {
        let stream = acceptor.accept(stream).await?;
        debug!("New SSL Accept: {}", address);
        let new_opt = opt.clone();
        async_std::task::spawn(async { if let anyhow::Result::Err(error) = proxy_ssl(new_opt, stream).await { warn!("Proxy Tcp Up Stream error: {}", error); } });
    }
    anyhow::Result::Ok(())
}

// handle ssl one
async fn proxy_ssl(opt: Opt, stream: async_tls::server::TlsStream<async_std::net::TcpStream>) -> anyhow::Result<()> {
    let (sender, receiver): (async_channel::Sender<crate::opt::StreamType>, async_channel::Receiver<crate::opt::StreamType>) = async_channel::bounded(1);
    let timeout = opt.timeout;
    match async_tungstenite::accept_hdr_async(stream, super::tcp::TheCallback(sender, opt)).timeout(std::time::Duration::from_secs(timeout)).await? {
        Err(error) => {
            warn!("Could Not Accept Web Socket Security: {}", error);
        },
        Ok(stream) => {
            let stream_type= receiver.recv().await?;
            debug!("New Web Socket Proxy to: {:?}", stream_type);
            match stream_type {
                crate::opt::StreamType::UnixDomainSocket(dst) => async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_ssl_uds_up(dst, stream).await { warn!("Proxy SSL Up Stream Error: {}", error) } }),
                crate::opt::StreamType::TcpSocket(dst) => async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_ssl_up(dst, stream).await { warn!("Proxy SSL Up Stream Error: {}", error) } }),
            };
        }
    }
    anyhow::Result::Ok(())
}

// client to server
async fn proxy_ssl_up(dst: String, stream: async_tungstenite::WebSocketStream<async_tls::server::TlsStream<async_std::net::TcpStream>>) -> anyhow::Result<()> {
    // client stream
    let (sink, mut client) = stream.split();
    // server stream
    let mut server = async_std::net::TcpStream::connect(dst).await?;
    let stream = server.clone();
    // server -> client
    async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_ssl_down(stream, sink).await { warn!("Proxy SSL Down Stream Error: {}", error) } });
    // client -> server
    while let Some(message) = client.next().await {
        let message = message?;
        if message.is_binary() || message.is_text() {
            debug!("Web Socket Security Proxy Up Message");
            server.write_all(&message.into_data()).await?;
        }
    }
    debug!("Proxy SSL Client Closed");
    server.shutdown(std::net::Shutdown::Both)?;
    anyhow::Result::Ok(())
}

// server to client
async fn proxy_ssl_down(mut stream: async_std::net::TcpStream, mut sink: futures_util::stream::SplitSink<async_tungstenite::WebSocketStream<async_tls::server::TlsStream<async_std::net::TcpStream>>, tungstenite::Message>) -> anyhow::Result<()> {
    // forward stream
    loop {
        let mut data = vec![0; 1024];
        let size = stream.read(&mut data).await?;
        if 0 < size {
            debug!("Web Socket Security UDS Proxy Down Message");
            sink.send(tungstenite::Message::Binary(data[..size].to_vec())).await?;
        } else {
            debug!("Proxy SSL Server Closed");
            sink.close().await?;
            break;
        }
    }
    anyhow::Result::Ok(())
}

// client to server
async fn proxy_ssl_uds_up(dst: String, stream: async_tungstenite::WebSocketStream<async_tls::server::TlsStream<async_std::net::TcpStream>>) -> anyhow::Result<()> {
    // client stream
    let (sink, mut client) = stream.split();
    // server stream
    let mut server = async_std::os::unix::net::UnixStream::connect(dst).await?;
    let stream = server.clone();
    // server -> client
    async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_ssl_uds_down(stream, sink).await { warn!("Proxy SSL Down Stream Error: {}", error) } });
    // client -> server
    while let Some(message) = client.next().await {
        let message = message?;
        if message.is_binary() || message.is_text() {
            debug!("Web Socket Security UDS Proxy Up Message");
            server.write_all(&message.into_data()).await?;
        }
    }
    debug!("Proxy SSL UDS Client Closed");
    server.shutdown(std::net::Shutdown::Both)?;
    anyhow::Result::Ok(())
}

// server to client
async fn proxy_ssl_uds_down(mut stream: async_std::os::unix::net::UnixStream, mut sink: futures_util::stream::SplitSink<async_tungstenite::WebSocketStream<async_tls::server::TlsStream<async_std::net::TcpStream>>, tungstenite::Message>) -> anyhow::Result<()> {
    // forward stream
    loop {
        let mut data = vec![0; 1024];
        let size = stream.read(&mut data).await?;
        if 0 < size {
            debug!("Web Socket Security Proxy Down Message");
            sink.send(tungstenite::Message::Binary(data[..size].to_vec())).await?;
        } else {
            debug!("Proxy SSL UDS Server Closed");
            sink.close().await?;
            break;
        }
    }
    anyhow::Result::Ok(())
}

// async tls acceptor
fn load_tls_acceptor(opt: &Opt) -> anyhow::Result<async_tls::TlsAcceptor> {
    let config = load_config(opt)?;
    let acceptor = async_tls::TlsAcceptor::from(std::sync::Arc::new(config));
    anyhow::Result::Ok(acceptor)
}

// async tls server config
fn load_config(opt: &Opt) -> anyhow::Result<rustls::ServerConfig> {
    let cert = load_cert(opt)?;
    let key = load_key(opt)?;
    let mut config= rustls::ServerConfig::new(rustls::NoClientAuth::new());
    config.set_single_cert(cert, key)?;
    anyhow::Result::Ok(config)
}

// ssl cert file
fn load_cert(opt: &Opt) -> anyhow::Result<Vec<rustls::Certificate>> {
    let file = std::fs::File::open(&opt.ssl_cert)?;
    let mut reader = std::io::BufReader::new(file);
    let cert = rustls::internal::pemfile::certs(&mut reader).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid SSL Cert"))?;
    anyhow::Result::Ok(cert)
}

// ssl pkcs8 private key
fn load_key(opt: &Opt) -> anyhow::Result<rustls::PrivateKey> {
    let file = std::fs::File::open(&opt.ssl_key)?;
    let mut reader = std::io::BufReader::new(file);
    let mut key = rustls::internal::pemfile::pkcs8_private_keys(&mut reader).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid SSL Key"))?;
    anyhow::Result::Ok(key.remove(0))
}
