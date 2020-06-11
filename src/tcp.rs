//! TCP Proxy

#![allow(irrefutable_let_patterns)]
use anyhow::Context;
use futures_util::io::AsyncReadExt;
use futures_util::io::AsyncWriteExt;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use async_std::prelude::FutureExt;
use crate::opt::Opt;

pub struct TheCallback(pub async_channel::Sender<crate::opt::StreamType>, pub Opt);

impl tungstenite::handshake::server::Callback for TheCallback {
    fn on_request(self, request: &tungstenite::handshake::server::Request, response: tungstenite::handshake::server::Response) -> Result<tungstenite::handshake::server::Response, tungstenite::handshake::server::ErrorResponse> {
        // extract port or file
        let result = match self.1.port_map_type {
            crate::opt::PortMapType::Domain => extract_from_host(request, &self.1),
            crate::opt::PortMapType::Prefix => extract_from_uri(request, &self.1),
        };
        // handle result
        return match result {
            anyhow::Result::Err(error) => {
                warn!("Could not extract port: {}", error);
                Result::Err(tungstenite::handshake::server::ErrorResponse::new(Some(error.to_string())))
            },
            anyhow::Result::Ok(msg) => {
                let _ = async_std::task::spawn(async move { self.0.send(msg).await });
                Result::Ok(response)
            }
        }
    }
}

// 12345.example.com => 127.0.0.1:12345
// 12345.example.com/12345.sock/ => /tmp/websockify/12345
fn extract_from_host(request: &tungstenite::handshake::server::Request, opt: &Opt) -> anyhow::Result<crate::opt::StreamType> {
    let value = request.headers().get("Host").context("Could not found Host field from headers")?;
    let value = value.to_str().context("Could not get Host string")?;
    let mut list: Vec<&str> = value.split(".").collect();
    // list always non empty after split and collect
    let dst = list.remove(0).trim();
    opt.get_stream_type(dst)
}

// example.com/12345 => 127.0.0.1:12345
// example.com/12345.sock/ => /tmp/websockify/12345.sock
fn extract_from_uri(request: &tungstenite::handshake::server::Request, opt: &Opt) -> anyhow::Result<crate::opt::StreamType> {
    let uri = request.uri().to_string();
    let mut list: Vec<&str> = uri.trim_matches('/').split("/").collect();
    // list always non empty after split and collect
    let dst = list.remove(list.len() - 1).trim();
    opt.get_stream_type(dst)
}

// tcp server entry
pub async fn run_tcp(opt: Opt) -> anyhow::Result<()> {
    // server
    let socket = async_std::net::TcpListener::bind(format!("{}:{}", opt.listen, opt.tcp_port)).await?;
    debug!("Bind: {}:{}", opt.listen, opt.tcp_port);
    // acceptor stream
    while let (stream, address) = socket.accept().await? {
        debug!("New Accept: {}", address);
        let new_opt = opt.clone();
        async_std::task::spawn(async { if let anyhow::Result::Err(error) = proxy_tcp(new_opt, stream).await { warn!("Proxy Tcp Up Stream error: {}", error); } });
    }
    anyhow::Result::Ok(())
}

// handle tcp one
async fn proxy_tcp(opt: Opt, stream: async_std::net::TcpStream) -> anyhow::Result<()> {
    let (sender, receiver): (async_channel::Sender<crate::opt::StreamType>, async_channel::Receiver<crate::opt::StreamType>) = async_channel::bounded(1);
    let timeout = opt.timeout;
    match async_tungstenite::accept_hdr_async(stream, TheCallback(sender, opt)).timeout(std::time::Duration::from_secs(timeout)).await? {
        Err(error) => {
            warn!("Could Not Accept Web Socket: {}", error);
        },
        Ok(stream) => {
            let stream_type = receiver.recv().await?;
            debug!("New Web Socket Proxy to: {:?}", stream_type);
            match stream_type {
                crate::opt::StreamType::UnixDomainSocket(dst) => async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_tcp_uds_up(dst, stream).await { warn!("Proxy Tcp Up Stream error: {}", error) } }),
                crate::opt::StreamType::TcpSocket(dst) => async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_tcp_up(dst, stream).await { warn!("Proxy Tcp Up Stream error: {}", error) } }),
            };
        }
    }
    anyhow::Result::Ok(())
}

// client to server
async fn proxy_tcp_up(dst: String, stream: async_tungstenite::WebSocketStream<async_std::net::TcpStream>) -> anyhow::Result<()> {
    // client stream
    let (sink, mut client) = stream.split();
    // server stream
    let mut server = async_std::net::TcpStream::connect(dst).await?;
    let stream = server.clone();
    // server -> client
    async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_tcp_down(stream, sink).await { warn!("Proxy Tcp Down Stream error: {}", error) } });
    // client -> server
    while let Some(message) = client.next().await {
        let message = message?;
        if message.is_binary() || message.is_text() {
            debug!("Web Socket Proxy Up Message");
            server.write_all(&message.into_data()).await?;
        }
    }
    debug!("Proxy Client Closed");
    server.shutdown(std::net::Shutdown::Both)?;
    anyhow::Result::Ok(())
}

// server to client
async fn proxy_tcp_down(mut stream: async_std::net::TcpStream, mut sink: futures_util::stream::SplitSink<async_tungstenite::WebSocketStream<async_std::net::TcpStream>, tungstenite::Message>) -> anyhow::Result<()> {
    // forward stream
    loop {
        let mut data = vec![0; 1024];
        let size = stream.read(&mut data).await?;
        if 0 < size {
            debug!("Web Socket Proxy Down Message");
            sink.send(tungstenite::Message::Binary(data[..size].to_vec())).await?;
        } else {
            debug!("Proxy Server Closed");
            sink.close().await?;
            break;
        }
    }
    anyhow::Result::Ok(())
}

// client to server
async fn proxy_tcp_uds_up(dst: String, stream: async_tungstenite::WebSocketStream<async_std::net::TcpStream>) -> anyhow::Result<()> {
    // client stream
    let (sink, mut client) = stream.split();
    // server stream
    let mut server = async_std::os::unix::net::UnixStream::connect(dst).await?;
    let stream = server.clone();
    // server -> client
    async_std::task::spawn(async {  if let anyhow::Result::Err(error) = proxy_tcp_uds_down(stream, sink).await { warn!("Proxy Tcp Down Stream error: {}", error) } });
    // client -> server
    while let Some(message) = client.next().await {
        let message = message?;
        if message.is_binary() || message.is_text() {
            debug!("Web Socket UDS Proxy Up Message");
            server.write_all(&message.into_data()).await?;
        }
    }
    debug!("Proxy UDS Client Closed");
    server.shutdown(std::net::Shutdown::Both)?;
    anyhow::Result::Ok(())
}

// server to client
async fn proxy_tcp_uds_down(mut stream: async_std::os::unix::net::UnixStream, mut sink: futures_util::stream::SplitSink<async_tungstenite::WebSocketStream<async_std::net::TcpStream>, tungstenite::Message>) -> anyhow::Result<()> {
    // forward stream
    loop {
        let mut data = vec![0; 1024];
        let size = stream.read(&mut data).await?;
        if 0 < size {
            debug!("Web Socket UDS Proxy Down Message");
            sink.send(tungstenite::Message::Binary(data[..size].to_vec())).await?;
        } else {
            debug!("Proxy UDS Server Closed");
            sink.close().await?;
            break;
        }
    }
    anyhow::Result::Ok(())
}
