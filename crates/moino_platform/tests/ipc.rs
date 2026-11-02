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

fn connect_when_ready(name: &str, child: &mut Server, deadline: Instant) -> impl Read + Write {
    loop {
        if let Ok(stream) = connect(name) {
            return stream;
        }
        assert!(Instant::now() < deadline, "server did not become ready");
        assert!(
            child.0.try_wait().expect("poll child").is_none(),
            "server exited early"
        );
        std::thread::yield_now();
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
    let mut stream = connect_when_ready(&name, &mut child, deadline);
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
// Spec: docs/acceptance/m0.md#技术原型
fn malformed_and_oversized_frames_are_rejected_over_real_transport_without_losing_the_listener() {
    let name = format!("moino-m0-invalid-{}", std::process::id());
    let mut child = Server(
        Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", "ipc_invalid_child", "--ignored"])
            .env("MOINO_TEST_SOCKET", &name)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("server process"),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    for frame in [u32::MAX.to_le_bytes().to_vec(), vec![1, 0, 0, 0, b'{']] {
        let mut stream = connect_when_ready(&name, &mut child, deadline);
        stream.write_all(&frame).expect("invalid frame sent");
        let mut ack = [0; 2];
        stream.read_exact(&mut ack).expect("rejection acknowledged");
        assert_eq!(&ack, b"NO");
    }
    let mut stream = connect_when_ready(&name, &mut child, deadline);
    send_request(&mut stream, &LaunchRequest::new(vec!["恢复.md".into()]))
        .expect("valid frame sent");
    let mut ack = [0; 2];
    stream
        .read_exact(&mut ack)
        .expect("valid request acknowledged");
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
        "endpoint released after rejected clients"
    );
}

#[test]
#[ignore = "spawned in an isolated child process by the parent integration test"]
fn ipc_invalid_child() {
    let name = std::env::var("MOINO_TEST_SOCKET").expect("fixture endpoint");
    let listener = listen(&name).expect("listen");
    let deadline = Instant::now() + Duration::from_secs(15);
    for invalid in [true, true, false] {
        let mut stream = loop {
            match accept(&listener) {
                Ok(stream) => break stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("accept failed: {error}"),
            }
            assert!(Instant::now() < deadline, "client did not connect");
            std::thread::yield_now();
        };
        let result = receive_request(&mut stream);
        if invalid {
            assert!(
                result.is_err(),
                "invalid frame must not reach launch handling"
            );
            stream.write_all(b"NO").expect("rejection");
        } else {
            assert_eq!(result.expect("request").paths(), ["恢复.md"]);
            stream.write_all(b"OK").expect("ack");
        }
    }
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
