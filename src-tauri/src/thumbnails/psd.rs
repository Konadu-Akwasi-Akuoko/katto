//! Minimal flattened PSD writer: 8BPS v1, RGB 8-bit, RLE white canvas, plus
//! Photoshop guide resources (image resource 0x0408) at the safe-zone lines.
//! Byte layout follows the Adobe "Photoshop File Formats" spec (header, image
//! resource blocks, grid & guides v1 with 32nds-of-a-pixel locations,
//! PackBits RLE). The owner may replace the committed outputs with
//! hand-authored PSDs under the same filenames.

/// A guide line at a pixel position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    Vertical(u32),
    Horizontal(u32),
}

/// What to write: canvas size plus guide lines.
pub struct PsdSpec {
    pub width: u32,
    pub height: u32,
    pub guides: Vec<Guide>,
}

/// 1280×720 YouTube thumbnail: 5% action-safe margins + center cross.
pub fn landscape_template() -> PsdSpec {
    PsdSpec {
        width: 1280,
        height: 720,
        guides: vec![
            Guide::Vertical(64),
            Guide::Vertical(640),
            Guide::Vertical(1216),
            Guide::Horizontal(36),
            Guide::Horizontal(360),
            Guide::Horizontal(684),
        ],
    }
}

/// 1080×1920 vertical: 10% top margin, center, 20% bottom (Shorts UI zone).
pub fn portrait_template() -> PsdSpec {
    PsdSpec {
        width: 1080,
        height: 1920,
        guides: vec![
            Guide::Vertical(54),
            Guide::Vertical(540),
            Guide::Vertical(1026),
            Guide::Horizontal(192),
            Guide::Horizontal(960),
            Guide::Horizontal(1536),
        ],
    }
}

fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// One white RLE scan line: runs of ≤128 repeated 0xFF bytes.
fn white_rle_row(width: u32) -> Vec<u8> {
    let mut row = Vec::new();
    let mut remaining = width;
    while remaining > 0 {
        let run = remaining.min(128);
        // PackBits repeat header: 1 - run length, as a signed byte
        row.push((1i16 - run as i16) as i8 as u8);
        row.push(0xFF);
        remaining -= run;
    }
    row
}

/// Serialize the spec as a flattened white RGB 8-bit PSD with guides.
pub fn write_psd(spec: &PsdSpec) -> Vec<u8> {
    let mut out = Vec::new();

    // file header (26 bytes)
    out.extend_from_slice(b"8BPS");
    push_u16(&mut out, 1);
    out.extend_from_slice(&[0u8; 6]);
    push_u16(&mut out, 3); // channels: RGB
    push_u32(&mut out, spec.height);
    push_u32(&mut out, spec.width);
    push_u16(&mut out, 8); // depth
    push_u16(&mut out, 3); // color mode: RGB

    // color mode data section: empty
    push_u32(&mut out, 0);

    // image resources: one 0x0408 grid & guides block
    let mut guides_data = Vec::new();
    push_u32(&mut guides_data, 1); // version
    push_u32(&mut guides_data, 576); // grid cycle horizontal (spec default)
    push_u32(&mut guides_data, 576); // grid cycle vertical
    push_u32(&mut guides_data, spec.guides.len() as u32);
    for guide in &spec.guides {
        let (pos, direction) = match guide {
            Guide::Vertical(px) => (*px, 0u8),
            Guide::Horizontal(px) => (*px, 1u8),
        };
        push_u32(&mut guides_data, pos * 32); // 32nds of a pixel
        guides_data.push(direction);
    }
    let mut block = Vec::new();
    block.extend_from_slice(b"8BIM");
    push_u16(&mut block, 0x0408);
    block.extend_from_slice(&[0u8, 0u8]); // empty pascal name, even-padded
    push_u32(&mut block, guides_data.len() as u32);
    block.extend_from_slice(&guides_data);
    if guides_data.len() % 2 == 1 {
        block.push(0);
    }
    push_u32(&mut out, block.len() as u32);
    out.extend_from_slice(&block);

    // layer & mask section: empty (flattened)
    push_u32(&mut out, 0);

    // image data: RLE, row-count table then PackBits rows
    push_u16(&mut out, 1);
    let row = white_rle_row(spec.width);
    let rows = (spec.height as usize) * 3;
    for _ in 0..rows {
        push_u16(&mut out, row.len() as u16);
    }
    for _ in 0..rows {
        out.extend_from_slice(&row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(b: &[u8], i: usize) -> u16 {
        u16::from_be_bytes([b[i], b[i + 1]])
    }
    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
    }

    #[test]
    fn header_is_valid_rgb_psd() {
        let bytes = write_psd(&landscape_template());
        assert_eq!(&bytes[0..4], b"8BPS");
        assert_eq!(u16_at(&bytes, 4), 1, "version");
        assert_eq!(u16_at(&bytes, 12), 3, "channels");
        assert_eq!(u32_at(&bytes, 14), 720, "height");
        assert_eq!(u32_at(&bytes, 18), 1280, "width");
        assert_eq!(u16_at(&bytes, 22), 8, "depth");
        assert_eq!(u16_at(&bytes, 24), 3, "RGB mode");
    }

    #[test]
    fn guide_resource_encodes_positions_in_32nds() {
        let spec = PsdSpec {
            width: 100,
            height: 50,
            guides: vec![Guide::Vertical(10), Guide::Horizontal(25)],
        };
        let bytes = write_psd(&spec);
        // locate the 8BIM 0x0408 block after the color-mode section (len 0)
        let res_len_at = 26 + 4; // header + color mode data length field
        let res_start = res_len_at + 4;
        assert_eq!(&bytes[res_start..res_start + 4], b"8BIM");
        assert_eq!(u16_at(&bytes, res_start + 4), 0x0408);
        // pascal name: empty => 2 bytes (len 0 + pad)
        let data_len_at = res_start + 6 + 2;
        let data_at = data_len_at + 4;
        assert_eq!(u32_at(&bytes, data_at), 1, "guides resource version");
        assert_eq!(u32_at(&bytes, data_at + 12), 2, "guide count");
        assert_eq!(u32_at(&bytes, data_at + 16), 10 * 32, "vertical at 10px");
        assert_eq!(bytes[data_at + 20], 0, "vertical direction");
        assert_eq!(u32_at(&bytes, data_at + 21), 25 * 32, "horizontal at 25px");
        assert_eq!(bytes[data_at + 25], 1, "horizontal direction");
    }

    #[test]
    fn rle_image_data_covers_every_channel_row() {
        let spec = PsdSpec {
            width: 4,
            height: 3,
            guides: vec![],
        };
        let bytes = write_psd(&spec);
        // find the image data section by walking sections, not magic offsets
        let after_header = 26usize;
        let color_len = u32_at(&bytes, after_header) as usize;
        let res_at = after_header + 4 + color_len;
        let res_len = u32_at(&bytes, res_at) as usize;
        let layers_at = res_at + 4 + res_len;
        let layers_len = u32_at(&bytes, layers_at) as usize;
        assert_eq!(layers_len, 0, "flattened");
        let img_at = layers_at + 4 + layers_len;
        assert_eq!(u16_at(&bytes, img_at), 1, "RLE compression");
        let rows = 3 * 3usize;
        let counts: Vec<u16> = (0..rows)
            .map(|r| u16_at(&bytes, img_at + 2 + r * 2))
            .collect();
        let total: usize = counts.iter().map(|&c| c as usize).sum();
        assert_eq!(
            bytes.len(),
            img_at + 2 + rows * 2 + total,
            "no trailing garbage"
        );
        // each RLE row for 4 white bytes: [0xFD, 0xFF] => 2 bytes
        assert!(counts.iter().all(|&c| c == 2));
    }

    #[test]
    fn templates_have_expected_dimensions_and_guides() {
        let l = landscape_template();
        assert_eq!((l.width, l.height), (1280, 720));
        assert_eq!(l.guides.len(), 6);
        let p = portrait_template();
        assert_eq!((p.width, p.height), (1080, 1920));
        assert_eq!(p.guides.len(), 6);
    }

    #[test]
    #[ignore = "regenerates the committed bundled templates"]
    fn regenerate_bundled_templates() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/thumbnail-templates");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("thumb-1280x720.psd"),
            write_psd(&landscape_template()),
        )
        .unwrap();
        std::fs::write(
            dir.join("thumb-1080x1920.psd"),
            write_psd(&portrait_template()),
        )
        .unwrap();
    }
}
