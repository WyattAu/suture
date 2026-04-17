use std::net::SocketAddr;

pub struct GrpcServer {
    addr: SocketAddr,
}

impl GrpcServer {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn serve(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_server_creation() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = GrpcServer::new(addr);
        assert_eq!(server.addr(), addr);
    }

    #[test]
    fn test_grpc_server_ephemeral_port() {
        let addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let server = GrpcServer::new(addr);
        assert_eq!(server.addr().port(), 0);
    }

    #[tokio::test]
    async fn test_grpc_server_serve_returns_ok() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = GrpcServer::new(addr);
        let result = server.serve().await;
        assert!(result.is_ok());
    }
}
