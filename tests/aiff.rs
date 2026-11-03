use garrido_studio::{
    aiff,
    wav::{self, Depth},
};
use std::sync::atomic::AtomicBool;

fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = tag.to_vec();
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
    out
}
fn fixture(
    bits: u16,
    encoding: Option<&[u8; 4]>,
    channels: u16,
    pcm: &[u8],
    offset: u32,
    block: u32,
    extra: usize,
) -> Vec<u8> {
    let mut common = Vec::new();
    common.extend(channels.to_be_bytes());
    common.extend((pcm.len() as u32 / (channels as u32 * (bits as u32).div_ceil(8))).to_be_bytes());
    common.extend(bits.to_be_bytes());
    common.extend([0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);
    if let Some(encoding) = encoding {
        common.extend(encoding);
        common.extend([0, 0]);
    }
    let mut sound = Vec::new();
    sound.extend(offset.to_be_bytes());
    sound.extend(block.to_be_bytes());
    sound.resize(8 + offset as usize, 0);
    sound.extend(pcm);
    sound.resize(sound.len() + extra, 0);
    let mut payload = if encoding.is_some() {
        b"AIFC".to_vec()
    } else {
        b"AIFF".to_vec()
    };
    payload.extend(chunk(b"ANNO", b"x"));
    payload.extend(chunk(b"SSND", &sound));
    payload.extend(chunk(b"COMM", &common));
    if encoding.is_some() {
        payload.extend(chunk(b"FVER", &0xA2805140u32.to_be_bytes()));
    }
    let mut bytes = b"FORM".to_vec();
    bytes.extend((payload.len() as u32).to_be_bytes());
    bytes.extend(payload);
    bytes
}
#[test]
fn pcm_export_matches_independent_wav_quantization_at_all_supported_rates() {
    let input = [
        [-1., 1.],
        [0.125, -0.25],
        [0., 0.],
        [0.123456, -0.654321],
        [-0.8, 0.8],
    ];
    for rate in [8000, 11025, 22050, 44100, 48000, 88200, 96000, 192000] {
        for channels in [1, 2] {
            for depth in [Depth::Pcm16, Depth::Pcm24] {
                let bytes = aiff::encode(&input, rate, depth, channels, false).unwrap();
                let sample = aiff::decode(&bytes).unwrap();
                let expected = wav::decode(
                    &wav::encode_channels(&input, rate, depth, false, channels).unwrap(),
                )
                .unwrap();
                assert_eq!(sample, expected);
                if rate == 44100 {
                    assert_eq!(&bytes[28..38], &[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);
                }
                assert_eq!(bytes.len() % 2, 0);
            }
        }
    }
}
#[test]
fn independent_pcm_vectors_cover_padding_offset_order_and_aifc_byte_order() {
    let bytes = fixture(12, None, 1, &[0xab, 0xc0, 0x40, 0], 3, 0, 0);
    let decoded = aiff::decode(&bytes).unwrap();
    assert_eq!(decoded.rate, 44100);
    assert_eq!(&*decoded.frames, &[[-1348. / 2048.; 2], [0.5; 2]]);
    let bytes = fixture(16, Some(b"sowt"), 2, &[0, 0x80, 0, 0x40], 0, 16, 12);
    assert_eq!(&*aiff::decode(&bytes).unwrap().frames, &[[-1., 0.5]]);
    let bytes = fixture(24, Some(b"NONE"), 2, &[0xc0, 0, 0, 0x20, 0, 0], 0, 0, 0);
    assert_eq!(&*aiff::decode(&bytes).unwrap().frames, &[[-0.5, 0.25]]);
    let bytes = fixture(24, Some(b"in24"), 2, &[0xc0, 0, 0, 0x20, 0, 0], 0, 0, 0);
    assert_eq!(&*aiff::decode(&bytes).unwrap().frames, &[[-0.5, 0.25]]);
    let bytes = fixture(
        32,
        Some(b"in32"),
        2,
        &[0xc0, 0, 0, 0, 0x20, 0, 0, 0],
        0,
        0,
        0,
    );
    assert_eq!(&*aiff::decode(&bytes).unwrap().frames, &[[-0.5, 0.25]]);
    let mut bad = fixture(12, None, 1, &[0x40, 1], 0, 0, 0);
    assert!(aiff::decode(&bad).is_err());
    bad[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(aiff::decode(&bad).is_err());
}
#[test]
fn aifc_float_is_finite_bounded_and_requires_its_version() {
    let pcm: Vec<_> = [0.25f32, -0.125]
        .into_iter()
        .flat_map(f32::to_be_bytes)
        .collect();
    let mut bytes = fixture(32, Some(b"fl32"), 2, &pcm, 0, 0, 0);
    assert_eq!(&*aiff::decode(&bytes).unwrap().frames, &[[0.25, -0.125]]);
    let len = bytes.len();
    bytes[len - 1] ^= 1;
    assert!(aiff::decode(&bytes).is_err());
    bytes.truncate(len - 12);
    let len = bytes.len() as u32 - 8;
    bytes[4..8].copy_from_slice(&len.to_be_bytes());
    assert!(aiff::decode(&bytes).is_err());
    let pcm: Vec<_> = [f32::NAN, 0.]
        .into_iter()
        .flat_map(f32::to_be_bytes)
        .collect();
    assert!(aiff::decode(&fixture(32, Some(b"fl32"), 2, &pcm, 0, 0, 0)).is_err());
    assert!(aiff::decode(&fixture(16, Some(b"ima4"), 2, &[0; 4], 0, 0, 0)).is_err());
}
#[test]
fn truncation_duplicate_chunks_and_declared_frame_exhaustion_are_rejected() {
    let bytes = aiff::encode(&[[0.1, -0.1]; 17], 48000, Depth::Pcm24, 2, false).unwrap();
    for end in 0..bytes.len() {
        assert!(aiff::decode(&bytes[..end]).is_err(), "{end}");
    }
    let mut invalid = bytes.clone();
    invalid[22..26].copy_from_slice(&4_000_001u32.to_be_bytes());
    assert!(aiff::decode(&invalid).is_err());
    invalid = bytes.clone();
    invalid[28..30].copy_from_slice(&0x7fffu16.to_be_bytes());
    assert!(aiff::decode(&invalid).is_err());
    invalid = bytes.clone();
    invalid.extend(&bytes[12..38]);
    let len = invalid.len() as u32 - 8;
    invalid[4..8].copy_from_slice(&len.to_be_bytes());
    assert!(aiff::decode(&invalid).is_err());
    for index in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[index] ^= 0xff;
        let _ = aiff::decode(&changed);
    }
}
#[test]
fn export_refuses_nonfinite_float_mode_and_cancellation() {
    assert!(aiff::encode(&[[f32::INFINITY, 0.]], 48000, Depth::Pcm24, 2, false).is_err());
    assert!(aiff::encode(&[[0.; 2]], 48000, Depth::Float32, 2, false).is_err());
    assert!(aiff::encode(&[[0.; 2]], 0, Depth::Pcm24, 2, false).is_err());
    assert!(
        aiff::encode_cancel(&[], 48000, Depth::Pcm24, 2, false, &AtomicBool::new(true)).is_err()
    );
}
