pub(super) struct Digest {
    state: [u32; 4],
    block: [u8; 64],
    used: usize,
    length: u64,
}
impl Digest {
    pub(super) fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            block: [0; 64],
            used: 0,
            length: 0,
        }
    }
    pub(super) fn update(&mut self, mut bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        while !bytes.is_empty() {
            let count = (64 - self.used).min(bytes.len());
            self.block[self.used..self.used + count].copy_from_slice(&bytes[..count]);
            self.used += count;
            bytes = &bytes[count..];
            if self.used == 64 {
                self.compress();
                self.used = 0;
            }
        }
    }
    pub(super) fn finish(mut self) -> [u8; 16] {
        let bits = self.length.wrapping_mul(8).to_le_bytes();
        self.update(&[0x80]);
        let zeros = if self.used <= 56 {
            56 - self.used
        } else {
            120 - self.used
        };
        self.update(&[0; 64][..zeros]);
        self.update(&bits);
        let mut result = [0; 16];
        for (word, bytes) in self.state.iter().zip(result.as_chunks_mut::<4>().0) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        result
    }
    fn compress(&mut self) {
        const SHIFT: [[u32; 4]; 4] = [
            [7, 12, 17, 22],
            [5, 9, 14, 20],
            [4, 11, 16, 23],
            [6, 10, 15, 21],
        ];
        const TABLE: [u32; 64] = [
            0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
            0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
            0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
            0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
            0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
            0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
            0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
            0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
            0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
            0xeb86d391,
        ];
        let mut words = [0u32; 16];
        for (word, bytes) in words.iter_mut().zip(self.block.as_chunks::<4>().0) {
            *word = u32::from_le_bytes(*bytes);
        }
        let [mut a, mut b, mut c, mut d] = self.state;
        for (i, constant) in TABLE.iter().enumerate() {
            let (function, index) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let next = b.wrapping_add(
                a.wrapping_add(function)
                    .wrapping_add(*constant)
                    .wrapping_add(words[index])
                    .rotate_left(SHIFT[i / 16][i % 4]),
            );
            (a, b, c, d) = (d, next, b, c);
        }
        for (state, word) in self.state.iter_mut().zip([a, b, c, d]) {
            *state = state.wrapping_add(word);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Digest;
    #[test]
    fn published_vectors_and_block_boundaries() {
        for (message, expected) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ] {
            for chunk in 1..=65 {
                let mut digest = Digest::new();
                for part in message.as_bytes().chunks(chunk) {
                    digest.update(part);
                }
                let result: String = digest.finish().iter().map(|b| format!("{b:02x}")).collect();
                assert_eq!(result, expected);
            }
        }
    }
}
