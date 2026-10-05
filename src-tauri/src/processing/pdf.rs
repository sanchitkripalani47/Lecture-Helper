use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// One slide image to place on its own PDF page.
///
/// Dimensions are read while scanning frames so the writer never has to decode
/// the JPEG again; only the bytes are streamed into the file.
#[derive(Debug, Clone)]
pub struct SlideImage {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub components: u8,
}

impl SlideImage {
    pub fn from_path(path: PathBuf) -> io::Result<Self> {
        let bytes = fs::read(&path)?;
        let (width, height, components) = jpeg_metadata(&bytes).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} is not a readable JPEG frame", path.display()),
            )
        })?;
        Ok(Self {
            path,
            width,
            height,
            components,
        })
    }

    fn color_space(&self) -> &'static str {
        if self.components <= 1 {
            "/DeviceGray"
        } else {
            "/DeviceRGB"
        }
    }
}

/// A4 portrait, in PDF points.
const MAX_PAGE_WIDTH: f64 = 595.0;
const MAX_PAGE_HEIGHT: f64 = 842.0;

fn fit_to_page(width: u32, height: u32) -> (f64, f64) {
    if width == 0 || height == 0 {
        return (MAX_PAGE_WIDTH, MAX_PAGE_HEIGHT);
    }
    let scale = (MAX_PAGE_WIDTH / width as f64).min(MAX_PAGE_HEIGHT / height as f64);
    (width as f64 * scale, height as f64 * scale)
}

struct CountingWriter<W: Write> {
    inner: W,
    offset: u64,
}

impl<W: Write> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, offset: 0 }
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.inner.write_all(bytes)?;
        self.offset += bytes.len() as u64;
        Ok(())
    }

    fn write_str(&mut self, text: &str) -> io::Result<()> {
        self.write_bytes(text.as_bytes())
    }
}

/// Write one page per slide, embedding the JPEG frames with `/DCTDecode`.
///
/// Building the file directly keeps the app free of a heavyweight PDF
/// dependency and gives full control over the byte layout.
pub fn write_pdf(destination: &Path, slides: &[SlideImage]) -> io::Result<u32> {
    if slides.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no slides were detected in this video",
        ));
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(destination)?;
    let mut writer = CountingWriter::new(BufWriter::new(file));
    let object_count = 2 + slides.len() * 3;
    let mut offsets: Vec<u64> = vec![0; object_count];

    // The comment line's four high bytes mark the file as binary for tools that
    // sniff the header, so they are written as raw bytes rather than UTF-8.
    writer.write_str("%PDF-1.4\n%")?;
    writer.write_bytes(&[0xE2, 0xE3, 0xCF, 0xD3, 0x0A])?;

    // 1: Catalog
    offsets[0] = writer.offset;
    writer.write_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;

    // 2: Page tree
    let mut kids = String::new();
    for index in 0..slides.len() {
        let page_id = 3 + index * 3;
        kids.push_str(&format!("{page_id} 0 R "));
    }
    offsets[1] = writer.offset;
    writer.write_str(&format!(
        "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
        kids.trim_end(),
        slides.len()
    ))?;

    for (index, slide) in slides.iter().enumerate() {
        let page_id = 3 + index * 3;
        let content_id = page_id + 1;
        let image_id = page_id + 2;

        let (page_width, page_height) = fit_to_page(slide.width, slide.height);
        let content = format!(
            "q\n{:.2} 0 0 {:.2} 0 0 cm\n/Im0 Do\nQ\n",
            page_width, page_height
        );

        offsets[page_id - 1] = writer.offset;
        writer.write_str(&format!(
            "{page_id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << /XObject << /Im0 {image_id} 0 R >> >> /Contents {content_id} 0 R >>\nendobj\n",
            page_width, page_height
        ))?;

        offsets[content_id - 1] = writer.offset;
        writer.write_str(&format!(
            "{content_id} 0 obj\n<< /Length {} >>\nstream\n",
            content.len()
        ))?;
        writer.write_str(&content)?;
        writer.write_str("endstream\nendobj\n")?;

        offsets[image_id - 1] = writer.offset;
        writer.write_str(&format!(
            "{image_id} 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            slide.width,
            slide.height,
            slide.color_space(),
            fs::metadata(&slide.path)?.len()
        ))?;

        let bytes = fs::read(&slide.path)?;
        writer.write_bytes(&bytes)?;
        writer.write_str("\nendstream\nendobj\n")?;
    }

    // Cross-reference table
    let xref_offset = writer.offset;
    writer.write_str(&format!("xref\n0 {}\n", object_count + 1))?;
    writer.write_str("0000000000 65535 f \n")?;
    for offset in offsets.iter().take(object_count) {
        writer.write_str(&format!("{:010} 00000 n \n", offset))?;
    }

    writer.write_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        object_count + 1,
        xref_offset
    ))?;

    writer.inner.flush()?;
    Ok(slides.len() as u32)
}

/// Read width, height, and component count from a JPEG's SOF marker.
pub fn jpeg_metadata(bytes: &[u8]) -> Option<(u32, u32, u8)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }

    let mut position = 2usize;
    while position + 3 < bytes.len() {
        if bytes[position] != 0xFF {
            position += 1;
            continue;
        }

        let marker = bytes[position + 1];
        position += 2;

        // Markers without a payload.
        if matches!(marker, 0xD8 | 0xD9 | 0x01) || (0xD0..=0xD7).contains(&marker) {
            continue;
        }

        if position + 1 >= bytes.len() {
            return None;
        }
        let length = (bytes[position] as usize) << 8 | bytes[position + 1] as usize;
        if length < 2 || position + length > bytes.len() {
            return None;
        }

        let is_start_of_frame = matches!(
            marker,
            0xC0 | 0xC1
                | 0xC2
                | 0xC3
                | 0xC5
                | 0xC6
                | 0xC7
                | 0xC9
                | 0xCA
                | 0xCB
                | 0xCD
                | 0xCE
                | 0xCF
        );

        if is_start_of_frame {
            let segment = &bytes[position + 2..position + length];
            if segment.len() < 6 {
                return None;
            }
            let height = ((segment[1] as u32) << 8) | segment[2] as u32;
            let width = ((segment[3] as u32) << 8) | segment[4] as u32;
            let components = segment[5];
            return Some((width, height, components));
        }

        // Skip the rest of this segment (length includes the two length bytes).
        position += length;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smallest valid baseline JPEG (1x1, grayscale) used to exercise parsing.
    const TINY_JPEG: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07,
        0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
        0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27,
        0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31, 0x34, 0x34, 0x34,
        0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B,
        0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
        0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0xFF, 0xDA, 0x00,
        0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x37, 0xFF, 0xD9,
    ];

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lecture-helper-pdf-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn reads_jpeg_dimensions_and_components() {
        let (width, height, components) = jpeg_metadata(TINY_JPEG).expect("metadata");
        assert_eq!((width, height, components), (1, 1, 1));
    }

    #[test]
    fn rejects_non_jpeg_bytes() {
        assert!(jpeg_metadata(b"not an image").is_none());
    }

    #[test]
    fn writes_a_readable_multipage_pdf() {
        let dir = scratch("write");
        let frame = dir.join("frame.jpg");
        fs::write(&frame, TINY_JPEG).expect("write frame");

        let slides = vec![SlideImage::from_path(frame.clone()).expect("slide")];
        let destination = dir.join("out/slides.pdf");
        let pages = write_pdf(&destination, &slides).expect("write pdf");
        assert_eq!(pages, 1);

        let bytes = fs::read(&destination).expect("read pdf");
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert!(bytes.ends_with(b"%%EOF\n"));
        assert!(String::from_utf8_lossy(&bytes).contains("/Filter /DCTDecode"));
    }

    #[test]
    fn refuses_to_write_a_pdf_with_no_slides() {
        let dir = scratch("empty");
        let result = write_pdf(&dir.join("empty.pdf"), &[]);
        assert!(result.is_err());
    }

    #[test]
    fn fits_landscape_slides_onto_the_page() {
        let (width, height) = fit_to_page(1920, 1080);
        assert!(width <= MAX_PAGE_WIDTH + 0.01);
        assert!(height <= MAX_PAGE_HEIGHT + 0.01);
        assert!((width / height - 1920.0 / 1080.0).abs() < 0.001);
    }
}
