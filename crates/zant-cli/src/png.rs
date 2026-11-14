//! Port of the parts of `src/core/png.zig` the CLI needs: PNG read / write
//! (delegated to the `png` crate), the area-average resize and the Windows
//! `.ico` builder, used for the app icons `zant package` / `zant build`
//! generate. The resize algorithm is a 1:1 port so icons match the Zig CLI.

/// Errors of `src/core/png.zig`: not a PNG, an unsupported PNG (bit depth /
/// interlacing) or corrupt data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NotPng,
    Unsupported,
    Corrupt,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotPng => f.write_str("NotPng"),
            Error::Unsupported => f.write_str("Unsupported"),
            Error::Corrupt => f.write_str("Corrupt"),
        }
    }
}

impl std::error::Error for Error {}

/// Straight (not premultiplied) RGBA, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// Width and height from the IHDR chunk (no decoding).
pub fn info(data: &[u8]) -> Result<(u32, u32), Error> {
    if data.len() < 33 || data[0..8] != SIGNATURE {
        return Err(Error::NotPng);
    }
    if &data[12..16] != b"IHDR" {
        return Err(Error::Corrupt);
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Ok((width, height))
}

/// Decodes an 8-bit, non-interlaced PNG into straight RGBA.
pub fn decode(data: &[u8]) -> Result<Image, Error> {
    let (width, height) = info(data)?;
    if data[28] != 0 {
        return Err(Error::Unsupported); // interlaced
    }
    let depth = data[24];
    if depth != 8 {
        return Err(Error::Unsupported);
    }
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(Error::Unsupported);
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::ALPHA | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(map_decode)?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or(Error::Unsupported)?];
    let frame = reader.next_frame(&mut buf).map_err(map_decode)?;
    let w = frame.width as usize;
    let h = frame.height as usize;
    let lines = frame.buffer_size() / h.max(1);
    let pixels = w * h;
    let mut rgba = Vec::with_capacity(pixels * 4);
    match frame.color_type {
        png::ColorType::Rgba => {
            for y in 0..h {
                let row = &buf[y * lines..y * lines + w * 4];
                rgba.extend_from_slice(row);
            }
        }
        png::ColorType::Rgb => {
            for y in 0..h {
                let row = &buf[y * lines..y * lines + w * 3];
                for px in row.chunks_exact(3) {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for y in 0..h {
                let row = &buf[y * lines..y * lines + w * 2];
                for px in row.chunks_exact(2) {
                    rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
                }
            }
        }
        png::ColorType::Grayscale => {
            for y in 0..h {
                let row = &buf[y * lines..y * lines + w];
                for px in row {
                    rgba.extend_from_slice(&[*px, *px, *px, 255]);
                }
            }
        }
        png::ColorType::Indexed => return Err(Error::Unsupported),
    }
    Ok(Image {
        width: frame.width,
        height: frame.height,
        rgba,
    })
}

fn map_decode(err: png::DecodingError) -> Error {
    match err {
        png::DecodingError::IoError(_) => Error::Corrupt,
        png::DecodingError::Format(_) => Error::Corrupt,
        png::DecodingError::Parameter(_) => Error::Corrupt,
        png::DecodingError::LimitsExceeded => Error::Unsupported,
    }
}

/// Area-average resize (each output pixel averages the source area it
/// covers), in premultiplied alpha so transparent edges do not darken.
pub fn resize(src: &Image, width: u32, height: u32) -> Image {
    let mut out = vec![0u8; width as usize * height as usize * 4];
    let sx = f64::from(src.width) / f64::from(width);
    let sy = f64::from(src.height) / f64::from(height);
    for oy in 0..height {
        let y0 = f64::from(oy) * sy;
        let y1 = y0 + sy;
        for ox in 0..width {
            let x0 = f64::from(ox) * sx;
            let x1 = x0 + sx;
            let mut acc = [0f64; 4];
            let mut total = 0f64;
            let mut y = y0.floor() as u32;
            while f64::from(y) < y1 && y < src.height {
                let wy = y1.min(f64::from(y + 1)) - y0.max(f64::from(y));
                let mut x = x0.floor() as u32;
                while f64::from(x) < x1 && x < src.width {
                    let wx = x1.min(f64::from(x + 1)) - x0.max(f64::from(x));
                    let w = wx * wy;
                    let p = (y as usize * src.width as usize + x as usize) * 4;
                    let p = &src.rgba[p..p + 4];
                    let a = f64::from(p[3]) / 255.0;
                    acc[0] += f64::from(p[0]) * a * w;
                    acc[1] += f64::from(p[1]) * a * w;
                    acc[2] += f64::from(p[2]) * a * w;
                    acc[3] += a * w;
                    total += w;
                    x += 1;
                }
                y += 1;
            }
            let o = (oy as usize * width as usize + ox as usize) * 4;
            let alpha = if total > 0.0 { acc[3] / total } else { 0.0 };
            if alpha <= 0.0 {
                continue;
            }
            for c in 0..3 {
                out[o + c] = (acc[c] / total / alpha).clamp(0.0, 255.0).round() as u8;
            }
            out[o + 3] = (alpha * 255.0).clamp(0.0, 255.0).round() as u8;
        }
    }
    Image {
        width,
        height,
        rgba: out,
    }
}

/// Shared output sink for `png::Encoder` (its `Writer` has no accessor).
#[derive(Clone, Default)]
struct SharedVec(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

impl std::io::Write for SharedVec {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// RGBA PNG.
pub fn encode(img: &Image) -> Result<Vec<u8>, Error> {
    let sink = SharedVec::default();
    let mut encoder = png::Encoder::new(sink.clone(), img.width, img.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(map_encode)?;
    writer.write_image_data(&img.rgba).map_err(map_encode)?;
    writer.finish().map_err(map_encode)?;
    let out = sink.0.borrow().clone();
    Ok(out)
}

fn map_encode(_err: png::EncodingError) -> Error {
    Error::Corrupt
}

/// Sizes in a Windows app `.ico` (Microsoft's app icon guidance: 16–256, with
/// the in-between sizes used at 125–200% display scaling).
pub const ICO_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

/// `.ico` with one PNG entry per size, resized from `src`.
pub fn build_ico(src: &Image) -> Result<Vec<u8>, Error> {
    let mut entries: Vec<Vec<u8>> = Vec::with_capacity(ICO_SIZES.len());
    for size in ICO_SIZES {
        let img = if src.width == size && src.height == size {
            src.clone()
        } else {
            resize(src, size, size)
        };
        entries.push(encode(&img)?);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(ICO_SIZES.len() as u16).to_le_bytes());
    let mut offset: u32 = 6 + 16 * ICO_SIZES.len() as u32;
    for (size, png) in ICO_SIZES.iter().zip(entries.iter()) {
        let mut dir = [0u8; 16];
        dir[0] = if *size >= 256 { 0 } else { *size as u8 };
        dir[1] = dir[0];
        dir[4..6].copy_from_slice(&1u16.to_le_bytes()); // planes
        dir[6..8].copy_from_slice(&32u16.to_le_bytes()); // bits per pixel
        dir[8..12].copy_from_slice(&(png.len() as u32).to_le_bytes());
        dir[12..16].copy_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&dir);
        offset += png.len() as u32;
    }
    for png in entries {
        out.extend_from_slice(&png);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard(width: u32, height: u32) -> Image {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&[
                    (x * 4) as u8,
                    128,
                    255,
                    if (x + y) % 2 == 0 { 255 } else { 0 },
                ]);
            }
        }
        Image {
            width,
            height,
            rgba,
        }
    }

    #[test]
    fn png_round_trip_resize_and_ico() {
        let img = checkerboard(8, 8);
        let encoded = encode(&img).expect("encode");
        assert_eq!(info(&encoded).expect("info"), (8, 8));
        let back = decode(&encoded).expect("decode");
        assert_eq!(back.rgba, img.rgba);
        let small = resize(&back, 4, 4);
        assert_eq!(small.width, 4);
        assert_eq!(small.height, 4);
        // half of the 2x2 block is opaque
        assert_eq!(small.rgba[3], 128);
        let ico = build_ico(&back).expect("ico");
        assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), ICO_SIZES.len() as u16);
        // first directory entry: 16x16, one plane, 32 bpp, data after the directory
        assert_eq!((ico[6], ico[7]), (16, 16));
        assert_eq!(u16::from_le_bytes([ico[10], ico[11]]), 1);
        assert_eq!(u16::from_le_bytes([ico[12], ico[13]]), 32);
        assert_eq!(
            u32::from_le_bytes([ico[18], ico[19], ico[20], ico[21]]),
            6 + 16 * ICO_SIZES.len() as u32
        );
    }

    #[test]
    fn rejects_non_png_data() {
        assert_eq!(
            info(b"not a png at all................"),
            Err(Error::NotPng)
        );
        assert_eq!(
            decode(b"not a png at all................"),
            Err(Error::NotPng)
        );
    }
}
