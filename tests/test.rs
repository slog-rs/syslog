extern crate slog;
extern crate slog_syslog;
extern crate syslog;

use slog::*;
use slog_syslog::*;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

mod test_server {
    use super::*;

    #[derive(Debug)]
    #[must_use = "the test server does nothing useful unless you send something to it"]
    pub struct TestServer {
        pub server_addr: SocketAddr,
        server_thread: Option<thread::JoinHandle<Vec<Box<[u8]>>>>,
        pub client_addr: SocketAddr,
    }

    impl TestServer {
        pub fn new() -> Self {
            let server_socket = UdpSocket::bind("localhost:0").expect("couldn't bind server socket");
            server_socket.set_read_timeout(Some(Duration::from_secs(10))).expect("couldn't set server socket read timeout");

            let server_addr = server_socket.local_addr().expect("couldn't get server socket address");

            let client_addr = {
                let mut client_addr = server_addr.clone();
                client_addr.set_port(0);
                client_addr
            };

            let server_thread = Some(thread::spawn(move || {        
                let mut packets = Vec::<Box<[u8]>>::new();
                let mut buf = [0u8; 65535];
        
                loop {
                    let (pkt_size, _) = server_socket.recv_from(&mut buf).expect("server couldn't receive packet");
        
                    if pkt_size == 4 && &buf[0..4] == b"STOP" {
                        break;
                    }
        
                    packets.push(Box::from(&buf[..pkt_size]));
                }
        
                packets
            }));

            TestServer { server_thread, server_addr, client_addr }
        }

        pub fn finish(mut self) -> Vec<Box<[u8]>> {
            let server_thread = self.server_thread.take().expect("server thread already stopped");

            {
                let client_socket = UdpSocket::bind(self.client_addr).expect("couldn't bind client socket");
                client_socket.send_to(b"STOP", &self.server_addr).expect("couldn't send stop packet");
            }

            server_thread.join().expect("server thread panicked")
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if self.server_thread.is_some() {
                // Try to stop the server thread. Ignore errors, since this will probably only happen when an error or panic has already occurred.
                if let Ok(client_socket) = UdpSocket::bind(self.client_addr) {
                    let _ = client_socket.send_to(b"STOP", &self.server_addr);
                }
            }
        }
    }
}
use test_server::TestServer;

#[test]
fn integration_test() {
    let server = TestServer::new();

    {
        // Set up a logger.
        let logger = Logger::root_typed(
            Mutex::new(Streamer3164::new(syslog::udp(
                syslog::Formatter3164 {
                    facility: Facility::LOG_USER,
                    hostname: Some("test-hostname".to_string()),
                    process: "test-app".to_string(),
                    pid: 123
                },
                &server.client_addr,
                &server.server_addr
            ).expect("couldn't create syslog logger"))).fuse(),
            o!("key" => "value")
        );

        // Log a test message.
        info!(logger, "Hello, world!"; "key2" => "value2");
    }

    // Get the logs received by the server thread.
    let logs = server.finish();

    // Check that the logs were correct.
    assert_eq!(logs.len(), 1);

    let s = String::from_utf8(logs[0].to_vec()).expect("log packet contains invalid UTF-8");
    assert!(s.starts_with("<14>"));
    assert!(s.ends_with("test-hostname test-app[123]: Hello, world! [key=\"value\" key2=\"value2\"]"));
}

#[test]
fn integration_test_with_builder() {
    let server = TestServer::new();

    {
        // Set up a logger.
        let drain = SyslogBuilder::new()
            .hostname("test-hostname")
            .process("test-app")
            .pid(123)
            .udp(server.client_addr, server.server_addr)
            .start()
            .expect("couldn't create syslog logger");

        let logger = Logger::root_typed(drain.fuse(), o!("key" => "value"));

        // Log a test message.
        info!(logger, "Hello, world!"; "key2" => "value2");
    }

    // Get the logs received by the server thread.
    let logs = server.finish();

    // Check that the logs were correct.
    assert_eq!(logs.len(), 1);

    let s = String::from_utf8(logs[0].to_vec()).expect("log packet contains invalid UTF-8");
    assert!(s.starts_with("<14>"));
    assert!(s.ends_with("test-hostname test-app[123]: Hello, world! [key=\"value\" key2=\"value2\"]"));
}

#[test]
fn integration_test_with_builder_and_msg_format() {
    let server = TestServer::new();

    {
        // Set up a logger.
        let drain = SyslogBuilder::new()
            .hostname("test-hostname")
            .process("test-app")
            .pid(123)
            .udp(server.client_addr, server.server_addr)
            .msg_format(NullMsgFormat3164)
            .start()
            .expect("couldn't create syslog logger");

        let logger = Logger::root_typed(drain.fuse(), o!("key" => "value"));

        // Log a test message.
        info!(logger, "Hello, world!"; "key2" => "value2");
    }

    // Get the logs received by the server thread.
    let logs = server.finish();

    // Check that the logs were correct.
    assert_eq!(logs.len(), 1);

    let s = String::from_utf8(logs[0].to_vec()).expect("log packet contains invalid UTF-8");
    assert!(s.starts_with("<14>"));
    assert!(s.ends_with("test-hostname test-app[123]: Hello, world!"));
}
