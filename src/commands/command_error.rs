pub fn make_error(msg: &str) -> serde_json::Value {
    return serde_json::json!({
        "error": msg
    });
}

pub fn make_error_bytes(msg: &str) -> Vec<u8> {
    return make_error(msg).to_string().as_bytes().to_vec();
}

pub fn make_error_bytes_with_prefix(prefix: Vec<u8>, msg: &str) -> Vec<u8> {
    let mut res = prefix.clone();
    res.append(&mut make_error(msg).to_string().as_bytes().to_vec());
    return res;
}