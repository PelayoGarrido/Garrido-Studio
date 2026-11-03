use garrido_studio::flac;
use std::sync::atomic::AtomicBool;

#[test]
fn flac_preserves_quantized_pcm_across_frame_boundaries() {
    let frames: Vec<_> = (0..8201)
        .map(|i| {
            [
                (((i * 71) % 60001) as f32 - 30000.) / 32768.,
                (((i * 97) % 60001) as f32 - 30000.) / 32768.,
            ]
        })
        .collect();
    for depth in [16, 24] {
        let data = flac::encode(&frames, 48000, depth, 2).unwrap();
        let decoded = flac::decode(&data).unwrap();
        assert_eq!(decoded.rate, 48000);
        assert_eq!(decoded.frames.as_ref(), &frames);
    }
}
#[test]
fn constant_and_predictable_pcm_compress_and_mono_is_correct() {
    for frames in [
        vec![[0.25, 0.75]; 8193],
        (0..8193).map(|i| [i as f32 / 65536.; 2]).collect(),
    ] {
        let data = flac::encode(&frames, 44100, 24, 1).unwrap();
        assert!(data.len() < frames.len() * 2);
        let decoded = flac::decode(&data).unwrap();
        assert_eq!(decoded.frames.len(), frames.len());
        for (a, b) in frames.iter().zip(decoded.frames.iter()) {
            assert_eq!((a[0] + a[1]) * 0.5, b[0]);
            assert_eq!(b[0], b[1]);
        }
    }
}
#[test]
fn prefixes_and_frame_crc_corruption_are_rejected() {
    let data = flac::encode(&[[0.25, -0.25]; 37], 8000, 16, 2).unwrap();
    for end in 0..data.len() {
        assert!(flac::decode(&data[..end]).is_err(), "prefix {end}");
    }
    let mut corrupt = data.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(flac::decode(&corrupt).is_err());
    let mut oversized = data;
    oversized[18..26].fill(255);
    assert!(flac::decode(&oversized).is_err());
}
#[test]
fn formats_limits_and_cancellation_fail_closed() {
    assert!(flac::encode(&[], 48000, 24, 2).is_err());
    assert!(flac::encode(&[[f32::NAN; 2]], 48000, 24, 2).is_err());
    assert!(flac::encode(&[[0.; 2]], 0, 24, 2).is_err());
    assert!(flac::encode(&[[0.; 2]], 48000, 32, 2).is_err());
    assert!(flac::encode_cancel(&[[0.; 2]], 48000, 24, 2, &AtomicBool::new(true)).is_err());
    for size in [1, 15, 16, 4096, 4097] {
        let encoded = flac::encode(&vec![[0.; 2]; size], 88200, 24, 2).unwrap();
        assert_eq!(flac::decode(&encoded).unwrap().frames.len(), size);
    }
}

#[test]
fn streaminfo_digest_matches_interleaved_signed_pcm_and_rejects_mismatch() {
    let frames = [[0.25, -0.5], [-1., 1.], [1. / 32768., -1. / 32768.]];
    for (depth, channels, expected) in [
        (16, 1, "07a20cc0e683491580f8be0413985ce4"),
        (16, 2, "45208236145ccd56da82ee640078205f"),
        (24, 1, "963f9d258b44c61d73270513f35ef06f"),
        (24, 2, "d9042c884c33925d151be517a8f4fddf"),
    ] {
        let mut data = flac::encode(&frames, 48000, depth, channels).unwrap();
        let digest: String = data[26..42].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(digest, expected);
        assert!(flac::decode(&data).is_ok());
        data[26] ^= 1;
        assert!(flac::decode(&data).is_err());
        data[26..42].fill(0);
        assert!(flac::decode(&data).is_ok());
    }
}

#[test]
fn rfc9639_example_three_decodes_lpc_and_verifies_its_digest() {
    let hex = "664c6143800000221000100000001f00001f07d0007000000018f8f9e396f5cbcfc6dc807f9977906b32fff868020017e944004f6f313d1047d227cb6d090831452bdc2822228057a3";
    let data: Vec<_> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect();
    let sample = flac::decode(&data).unwrap();
    let expected = [
        0, 79, 111, 78, 8, -61, -90, -68, -13, 42, 67, 53, 13, -27, -46, -38, -12, 14, 24, 19, 6,
        -4, -5, 0,
    ];
    assert_eq!(sample.rate, 32000);
    assert_eq!(sample.frames.len(), expected.len());
    for (frame, expected) in sample.frames.iter().zip(expected) {
        assert_eq!(*frame, [expected as f32 / 128.; 2]);
    }
}
