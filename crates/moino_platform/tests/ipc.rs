//! Real two-process transport coverage; no application timers are simulated here.
use moino_platform::{LaunchRequest, accept, connect, listen, receive_request, send_request};
use std::{
    io::{self, Read, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill(); /* Reap the isolated fixture even after a failed assertion. */
        let _ = self.0.wait(); /* Reap child resources. */
    }
}

#[test]
// Spec: docs/acceptance/m0.md#技术原型
fn second_process_forwards_and_endpoint_is_reusable() {
    let name = format!("moino-m0-{}", std::process::id());
    let mut child = Server(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", "ipc_child", "--ignored"])
            .env("MOINO_TEST_SOCKET", &name)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("server process"),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut stream = loop {
        if let Ok(stream) = connect(&name) {
            break stream;
        }
        assert!(Instant::now() < deadline, "server did not become ready");
        assert!(
            child.0.try_wait().expect("poll child").is_none(),
            "server exited early"
        );
        std::thread::yield_now();
    };
    send_request(&mut stream, &LaunchRequest::new(vec!["测试.md".into()])).expect("send");
    let mut ack = [0; 2];
    stream.read_exact(&mut ack).expect("ack");
    assert_eq!(&ack, b"OK");
    drop(stream);
    loop {
        if let Some(status) = child.0.try_wait().expect("poll exit") {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "server did not exit");
        std::thread::yield_now();
    }
    assert!(
        listen(&name).is_ok(),
        "endpoint released by server shutdown"
    );
}

#[test]
#[ignore = "spawned in an isolated child process by the parent integration test"]
fn ipc_child() {
    let name = std::env::var("MOINO_TEST_SOCKET").expect("fixture endpoint");
    let listener = listen(&name).expect("listen");
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut stream = loop {
        match accept(&listener) {
            Ok(stream) => break stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("accept failed: {error}"),
        }
        assert!(Instant::now() < deadline, "client did not connect");
        std::thread::yield_now();
    };
    let request = receive_request(&mut stream).expect("request");
    assert_eq!(request.paths(), ["测试.md"]);
    stream.write_all(b"OK").expect("ack");
}
