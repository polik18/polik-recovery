use data_recovery_file_carver::validate_carved_bytes;

fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB88320 & mask);
        }
    }
    crc
}

fn crc32_concat(a: &[u8], b: &[u8]) -> u32 {
    let crc = 0xFFFF_FFFFu32;
    let crc = crc32_update(crc, a);
    !crc32_update(crc, b)
}

fn png_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out.extend_from_slice(&crc32_concat(kind, payload).to_be_bytes());
    out
}

fn one_pixel_png() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    data.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
    // A tiny zlib stream is not decoded by the validator; CRC/chunk layout is the quality gate here.
    data.extend_from_slice(&png_chunk(
        b"IDAT",
        b"\x78\x9c\x63\x60\x60\x00\x00\x00\x03\x00\x01",
    ));
    data.extend_from_slice(&png_chunk(b"IEND", b""));
    data
}

fn minimal_pdf() -> Vec<u8> {
    let mut out = b"%PDF-1.4\n".to_vec();
    let obj1 = b"1 0 obj\n<< /Type /Catalog >>\nendobj\n";
    let obj_offset = out.len();
    out.extend_from_slice(obj1);
    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 2\n0000000000 65535 f \n");
    out.extend_from_slice(format!("{obj_offset:010} 00000 n \n").as_bytes());
    out.extend_from_slice(b"trailer\n<< /Root 1 0 R /Size 2 >>\nstartxref\n");
    out.extend_from_slice(xref_offset.to_string().as_bytes());
    out.extend_from_slice(b"\n%%EOF\n");
    out
}

#[test]
fn png_crc_validates_and_bad_crc_fails() {
    let good = one_pixel_png();
    let validation = validate_carved_bytes("PNG", &good);
    assert!(validation.ok);
    assert_eq!(validation.extension, "png");
    assert!(validation.confidence >= 0.90);

    let mut bad = good;
    let len = bad.len();
    bad[len - 1] ^= 0x55;
    let validation = validate_carved_bytes("PNG", &bad);
    assert!(!validation.ok);
}

#[test]
fn pdf_startxref_is_required() {
    let good = minimal_pdf();
    let validation = validate_carved_bytes("PDF", &good);
    assert!(validation.ok);
    assert_eq!(validation.extension, "pdf");

    let broken = b"%PDF-1.7\n1 0 obj<<>>endobj\n%%EOF\n";
    let validation = validate_carved_bytes("PDF", broken);
    assert!(!validation.ok);
}

#[test]
fn padding_is_not_a_valid_file() {
    let data = vec![0u8; 4096];
    let validation = validate_carved_bytes("PNG", &data);
    assert!(!validation.ok);
    assert_eq!(validation.confidence, 0.0);
}
