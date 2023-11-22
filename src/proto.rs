use hyper::ext::Protocol;
use log::debug;
use tokio_util::bytes::{Buf, BufMut, Bytes};

use crate::error::Box;

// max supported UDP packet size as per RFC 9298
pub const UDP_MAX: usize = 65527;

pub const CONNECT_UDP: Protocol = Protocol::from_static("connect-udp");

// Variable length integer decoding from RFC 9000.
pub fn decode_varint(b: &mut impl Buf) -> u64 {
    let mut n = b.get_u8() as u64;
    let prefix = n >> 6;
    let len = 1 << prefix;

    n &= 0b00111111;

    for _ in 1..len {
        n = (n << 8) + b.get_u8() as u64;
    }

    n
}

// Variable length integer encoding from RFC 9000.
pub fn encode_varint(b: &mut impl BufMut, n: u64) {
    if n <= 63 {
        b.put_u8(n as u8);
    } else if n <= 16383 {
        b.put_u16(0b01 << 14 | n as u16);
    } else if n <= 1073741823 {
        b.put_u32(0b10 << 30 | n as u32);
    } else if n <= 4611686018427387903 {
        b.put_u64(0b11 << 62 | n);
    } else {
        panic!("out of range");
    }
}

pub fn decode_capsule(b: &mut impl Buf) -> Result<Bytes, Box> {
    if decode_varint(b) != 0 {
        Err("unrecognized type")?
    }

    let len = decode_varint(b);
    debug!("decoding capsule with size {}", len);
    Ok(b.copy_to_bytes(len.try_into().unwrap()))
}

pub fn encode_capsule(b: &mut impl BufMut, payload: &[u8]) {
    debug!("encoding capsule with size {}", payload.len());

    encode_varint(b, 0);
    encode_varint(b, payload.len() as u64);
    b.put_slice(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test cases from RFC 9000.

    #[test]
    fn test_decode_varint() {
        for (k, v) in [
            (
                &mut &[0xc2, 0x19, 0x7c, 0x5e, 0xff, 0x14, 0xe8, 0x8c as u8][..],
                151288809941952652 as u64,
            ),
            (&mut &[0x9d, 0x7f, 0x3e, 0x7d][..], 494878333),
            (&mut &[0x7b, 0xbd][..], 15293),
            (&mut &[0x40, 0x25][..], 37),
            (&mut &[0x25][..], 37),
        ] {
            assert_eq!(decode_varint(k), v);
        }
    }

    #[test]
    fn test_encode_varint() {
        for (k, v) in [
            (
                &[0xc2, 0x19, 0x7c, 0x5e, 0xff, 0x14, 0xe8, 0x8c as u8][..],
                151288809941952652 as u64,
            ),
            (&[0x9d, 0x7f, 0x3e, 0x7d][..], 494878333),
            (&[0x7b, 0xbd][..], 15293),
            (&[0x25][..], 37),
        ] {
            let mut buf = Vec::new();
            encode_varint(&mut buf, v);
            assert_eq!(k, buf);
        }
    }
}
