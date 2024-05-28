use rand::Rng;

use crate::{Pcg, Sample};

pub fn gen_uuid(rng: &mut Pcg) -> Vec<Sample> {
    // https://datatracker.ietf.org/doc/html/rfc9562#section-5.4
    let mut uuid: [u8; 16] = rng.gen();
    uuid[6] = (uuid[6] & 0x0f) | 0x40; // version (byte 6 to hex 4x -> 0x40)
    uuid[8] = (uuid[8] & 0x3f) | 0x80; // variant (byte 8 to bin 10xx_xxxx -> 0x80)

    let bytes = format_uuid(uuid);
    let s = unsafe {
        // SAFETY: only ASCII used
        std::str::from_utf8_unchecked(&bytes)
    };
    vec![Sample::text(s.into())]
}

const HEX: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b', b'c', b'd', b'e', b'f',
];
const PARTS: [u8; 5] = [8, 4, 4, 4, 12];

fn format_uuid(uuid: [u8; 16]) -> [u8; 36] {
    let mut dst = [0; 36];

    let mut j = 0;
    let mut curr_part_len = 0;
    let mut p = 0;

    for b in uuid {
        let h = HEX[(b >> 4) as usize];
        let l = HEX[(b & 0x0f) as usize];
        dst[j] = h;
        dst[j + 1] = l;
        j += 2;
        curr_part_len += 2;
        if curr_part_len == PARTS[p] && p < PARTS.len() - 1 {
            p += 1;
            curr_part_len = 0;
            dst[j] = b'-';
            j += 1;
        }
    }
    dst
}
