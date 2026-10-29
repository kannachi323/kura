//! OneDrive's QuickXorHash, ported from Microsoft's reference implementation.
//! The digest is 20 bytes; OneDrive reports it base64-encoded.

const WIDTH_IN_BITS: usize = 160;
const SHIFT: usize = 11;
const BITS_IN_LAST_CELL: usize = 32;
const CELLS: usize = (WIDTH_IN_BITS - 1) / 64 + 1;

#[derive(Clone, Default)]
pub struct QuickXorHash {
    data: [u64; CELLS],
    length: u64,
    shift_so_far: usize,
}

impl QuickXorHash {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let mut cell = self.shift_so_far / 64;
        let mut offset = self.shift_so_far % 64;
        let iterations = bytes.len().min(WIDTH_IN_BITS);
        for i in 0..iterations {
            let is_last_cell = cell == CELLS - 1;
            let bits_in_cell = if is_last_cell { BITS_IN_LAST_CELL } else { 64 };
            if offset <= bits_in_cell - 8 {
                let mut j = i;
                while j < bytes.len() {
                    self.data[cell] ^= u64::from(bytes[j]) << offset;
                    j += WIDTH_IN_BITS;
                }
            } else {
                let next = if is_last_cell { 0 } else { cell + 1 };
                let low = bits_in_cell - offset;
                let mut xored = 0u8;
                let mut j = i;
                while j < bytes.len() {
                    xored ^= bytes[j];
                    j += WIDTH_IN_BITS;
                }
                self.data[cell] ^= u64::from(xored) << offset;
                self.data[next] ^= u64::from(xored) >> low;
            }
            offset += SHIFT;
            while offset >= bits_in_cell {
                cell = if is_last_cell { 0 } else { cell + 1 };
                offset -= bits_in_cell;
            }
        }
        self.shift_so_far =
            (self.shift_so_far + SHIFT * (bytes.len() % WIDTH_IN_BITS)) % WIDTH_IN_BITS;
        self.length = self.length.wrapping_add(bytes.len() as u64);
    }

    pub fn finalize(&self) -> [u8; 20] {
        let mut out = [0u8; 20];
        for (index, cell) in self.data.iter().enumerate() {
            let bytes = cell.to_le_bytes();
            let start = index * 8;
            let end = (start + 8).min(out.len());
            out[start..end].copy_from_slice(&bytes[..end - start]);
        }
        let length = self.length.to_le_bytes();
        let start = WIDTH_IN_BITS / 8 - length.len();
        for (index, byte) in length.iter().enumerate() {
            out[start + index] ^= byte;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine};

    use super::QuickXorHash;

    fn digest(input: &[u8]) -> String {
        let mut hash = QuickXorHash::new();
        hash.update(input);
        STANDARD.encode(hash.finalize())
    }

    #[test]
    fn matches_reference_vectors() {
        // From rclone's port of Microsoft's reference implementation.
        let vectors = [
            ("", "AAAAAAAAAAAAAAAAAAAAAAAAAAA="),
            ("Sg==", "SgAAAAAAAAAAAAAAAQAAAAAAAAA="),
            ("/aNzzCFPS/A=", "/RjFHJgRgicsAR4ACAAAAAAAAAA="),
            (
                "T6LYJIfDh81JrAK309H2JMJTXis=",
                "zBTHrspn3mEcohlJdIUAbjGNaNg=",
            ),
            (
                "DWAAX5/CIfrmErgZa8ot6ZraeSbu",
                "LR2Z0PjuRYGKQB/mhQAuMrAGZbQ=",
            ),
            (
                "0n7nl3YJtipy6yeUbVPWtc2h45WbF9u8hTz5tNwj3dZZwfXWkk+GN3g=",
                "YJLNK7JR64j9aODWfqDvEe/u6NU=",
            ),
            (
                "sa6SHK9z/G505bysK5KgRO2z2cTksDkLoFc7sv0tWBmf2G2mCiozf2Ce6EIO+W1fRsrrtn/eeOAV",
                "xZg1CwMNAjN0AIXw2yh4+1N3oos=",
            ),
        ];
        for (input, expected) in vectors {
            let bytes = STANDARD.decode(input).unwrap();
            assert_eq!(digest(&bytes), expected, "input {input}");
        }
    }

    #[test]
    fn chunking_does_not_change_the_digest() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i * 31 % 251) as u8).collect();
        let whole = digest(&data);
        for chunk in [1, 7, 160, 161, 4096] {
            let mut hash = QuickXorHash::new();
            for part in data.chunks(chunk) {
                hash.update(part);
            }
            assert_eq!(STANDARD.encode(hash.finalize()), whole, "chunk {chunk}");
        }
    }
}
