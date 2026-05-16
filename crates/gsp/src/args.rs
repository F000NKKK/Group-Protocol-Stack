//! Per-signal argument schema validation (gsp_rfc §6, §11).
//!
//! Each signal type has defined arg requirements:
//! - JOIN, LEAVE          — args MAY be empty; no required keys.
//! - ROLE_CHANGE          — args MUST be a CBOR map with keys 0 (target_member_id)
//!                          and 1 (new_role).
//! - MUTE, UNMUTE         — args MUST be a CBOR map with key 0 (target_member_id).
//! - STREAM_START,
//!   STREAM_STOP          — args MUST be a CBOR map with key 0 (stream_type).
//! - CODEC_UPDATE         — args MUST be a CBOR map with key 0 (codec_id).

use gbp_core::SignalType;

/// Validates the `args` bytes for the given signal type.
///
/// Returns `Ok(())` if the args conform to the schema, or an error string
/// describing the first violation. The error string is used by the caller to
/// populate `ERR_GSP_BAD_SCHEMA`.
pub fn validate_args(signal: SignalType, args: &[u8]) -> Result<(), &'static str> {
    match signal {
        // Membership signals that require no structured args.
        SignalType::Join | SignalType::Leave => Ok(()),

        // ROLE_CHANGE: {0: target_member_id (uint), 1: new_role (uint)}
        SignalType::RoleChange => {
            let map = decode_map(args).ok_or("ROLE_CHANGE: args must be a CBOR map")?;
            require_uint_key(&map, 0).ok_or("ROLE_CHANGE: missing key 0 (target_member_id)")?;
            require_uint_key(&map, 1).ok_or("ROLE_CHANGE: missing key 1 (new_role)")?;
            Ok(())
        }

        // MUTE / UNMUTE: {0: target_member_id (uint)}
        SignalType::Mute | SignalType::Unmute => {
            let map = decode_map(args).ok_or("MUTE/UNMUTE: args must be a CBOR map")?;
            require_uint_key(&map, 0).ok_or("MUTE/UNMUTE: missing key 0 (target_member_id)")?;
            Ok(())
        }

        // STREAM_START / STREAM_STOP: {0: stream_type (uint)}
        SignalType::StreamStart | SignalType::StreamStop => {
            let map = decode_map(args).ok_or("STREAM_START/STOP: args must be a CBOR map")?;
            require_uint_key(&map, 0).ok_or("STREAM_START/STOP: missing key 0 (stream_type)")?;
            Ok(())
        }

        // CODEC_UPDATE: {0: codec_id (uint)}
        SignalType::CodecUpdate => {
            let map = decode_map(args).ok_or("CODEC_UPDATE: args must be a CBOR map")?;
            require_uint_key(&map, 0).ok_or("CODEC_UPDATE: missing key 0 (codec_id)")?;
            Ok(())
        }
    }
}

/// Decodes a CBOR byte string as a map of `(uint key → ciborium::Value)`.
/// Returns `None` if the bytes are not a valid CBOR map.
fn decode_map(args: &[u8]) -> Option<Vec<(u64, ciborium::Value)>> {
    if args.is_empty() {
        return None;
    }
    let value: ciborium::Value = ciborium::from_reader(args).ok()?;
    let pairs = value.into_map().ok()?;
    let mut out = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        let key = match k {
            ciborium::Value::Integer(i) => u64::try_from(i).ok()?,
            _ => return None,
        };
        out.push((key, v));
    }
    Some(out)
}

/// Returns `Some(value)` if the map contains an integer key `k` whose value
/// is a CBOR unsigned integer, `None` otherwise.
fn require_uint_key(map: &[(u64, ciborium::Value)], k: u64) -> Option<u64> {
    map.iter().find(|(key, _)| *key == k).and_then(|(_, v)| {
        if let ciborium::Value::Integer(i) = v {
            u64::try_from(*i).ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cbor_map(pairs: &[(u64, u64)]) -> Vec<u8> {
        let map: Vec<(ciborium::Value, ciborium::Value)> = pairs
            .iter()
            .map(|(k, v)| {
                (
                    ciborium::Value::Integer((*k as u64).into()),
                    ciborium::Value::Integer((*v as u64).into()),
                )
            })
            .collect();
        let mut buf = Vec::new();
        ciborium::into_writer(&ciborium::Value::Map(map), &mut buf).unwrap();
        buf
    }

    #[test]
    fn join_leave_accept_empty_args() {
        assert!(validate_args(SignalType::Join, &[]).is_ok());
        assert!(validate_args(SignalType::Leave, &[]).is_ok());
    }

    #[test]
    fn role_change_valid() {
        let args = cbor_map(&[(0, 42), (1, 2)]);
        assert!(validate_args(SignalType::RoleChange, &args).is_ok());
    }

    #[test]
    fn role_change_missing_key_1() {
        let args = cbor_map(&[(0, 42)]);
        assert!(validate_args(SignalType::RoleChange, &args).is_err());
    }

    #[test]
    fn role_change_empty_args_rejected() {
        assert!(validate_args(SignalType::RoleChange, &[]).is_err());
    }

    #[test]
    fn mute_valid() {
        let args = cbor_map(&[(0, 7)]);
        assert!(validate_args(SignalType::Mute, &args).is_ok());
        assert!(validate_args(SignalType::Unmute, &args).is_ok());
    }

    #[test]
    fn mute_missing_target() {
        let args = cbor_map(&[(1, 99)]);
        assert!(validate_args(SignalType::Mute, &args).is_err());
    }

    #[test]
    fn stream_start_stop_valid() {
        let args = cbor_map(&[(0, 1)]);
        assert!(validate_args(SignalType::StreamStart, &args).is_ok());
        assert!(validate_args(SignalType::StreamStop, &args).is_ok());
    }

    #[test]
    fn stream_start_missing_stream_type() {
        assert!(validate_args(SignalType::StreamStart, &[]).is_err());
    }

    #[test]
    fn codec_update_valid() {
        let args = cbor_map(&[(0, 1)]);
        assert!(validate_args(SignalType::CodecUpdate, &args).is_ok());
    }

    #[test]
    fn codec_update_missing_codec_id() {
        assert!(validate_args(SignalType::CodecUpdate, &[]).is_err());
    }
}
