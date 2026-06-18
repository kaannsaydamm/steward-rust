use super::local_port;

#[test]
fn extracts_only_local_http_ports() {
    assert_eq!(local_port("http://127.0.0.1:50051"), Some(50051));
    assert_eq!(local_port("http://localhost:37241/"), Some(37241));
    assert_eq!(local_port("http://[::1]:50051"), Some(50051));
    assert_eq!(local_port("https://127.0.0.1:50051"), None);
    assert_eq!(local_port("http://192.168.1.10:50051"), None);
}
