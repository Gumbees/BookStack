use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    StateVector, Update,
};

// Protocol message type bytes.
pub const MSG_SYNC_STEP1: u8 = 0;
pub const MSG_SYNC_STEP2: u8 = 1;
pub const MSG_UPDATE: u8 = 2;
pub const MSG_AWARENESS: u8 = 3;

/// Encode a usize as a variable-length uint (7-bit little-endian).
pub fn encode_var_uint(mut n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
    out
}

/// Decode a variable-length uint. Returns (value, bytes_consumed).
pub fn decode_var_uint(data: &[u8]) -> Option<(usize, usize)> {
    let mut result: usize = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        result |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        if shift >= 64 {
            return None;
        }
    }
    None
}

/// Decode a length-prefixed byte slice.
pub fn decode_prefixed(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    let (len, consumed) = decode_var_uint(data)?;
    let start = consumed;
    let end = start + len;
    if end > data.len() {
        return None;
    }
    Some(data[start..end].to_vec())
}

/// Build a sync step 1 message from a state vector.
pub fn encode_sync_step1(sv: &StateVector) -> Vec<u8> {
    let sv_bytes = sv.encode_v1();
    let mut msg = vec![MSG_SYNC_STEP1];
    msg.extend_from_slice(&encode_var_uint(sv_bytes.len()));
    msg.extend_from_slice(&sv_bytes);
    msg
}

/// Build an update message from raw update bytes.
pub fn encode_update_message(update_bytes: &[u8]) -> Vec<u8> {
    let mut msg = vec![MSG_UPDATE];
    msg.extend_from_slice(&encode_var_uint(update_bytes.len()));
    msg.extend_from_slice(update_bytes);
    msg
}

/// Build an awareness message from a JSON payload.
pub fn encode_awareness_message(json_bytes: &[u8]) -> Vec<u8> {
    let mut msg = vec![MSG_AWARENESS];
    msg.extend_from_slice(&encode_var_uint(json_bytes.len()));
    msg.extend_from_slice(json_bytes);
    msg
}

/// Attempt to decode an Update from a step-2 or update message body.
pub fn decode_update(body: &[u8]) -> Option<Update> {
    Update::decode_v1(body).ok()
}

/// Attempt to decode a StateVector from a step-1 message body.
pub fn decode_state_vector(body: &[u8]) -> Option<StateVector> {
    StateVector::decode_v1(body).ok()
}
