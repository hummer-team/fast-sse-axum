pub mod tcp_configuration {
    use socket2::{Domain, Socket, Type};
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::net::TcpListener;

    /// create tcp listener
    pub fn create_tcp_listener(addr: SocketAddr) -> std::io::Result<TcpListener> {
        let socket = Socket::new(Domain::for_address(addr), Type::STREAM, None)?;
        socket.set_reuse_address(true)?;
        socket.set_tcp_nodelay(true)?;
        socket.set_linger(Some(Duration::from_secs(0)))?;
        #[cfg(target_os = "linux")]
        socket.set_tcp_fastopen(true)?;
        socket.bind(&addr.into())?;
        // backlog queue size
        socket.listen(128)?;
        let std_listener: std::net::TcpListener = socket.into();
        std_listener.set_nonblocking(true)?;
        TcpListener::from_std(std_listener)
    }
}
