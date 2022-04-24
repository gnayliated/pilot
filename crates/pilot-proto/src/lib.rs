use prost::Message;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/prometheus.rs"));
}

pub fn serialize_write_request(wq: &proto::WriteRequest) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(wq.encoded_len());
    wq.encode(&mut buf).unwrap();
    buf
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
