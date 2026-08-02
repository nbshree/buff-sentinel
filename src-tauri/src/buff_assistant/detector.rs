use image::{GrayImage, Luma, imageops};

#[derive(Clone)]
pub struct TemplateData {
    pub image: GrayImage,
    pub mask: GrayImage,
}

impl TemplateData {
    pub fn new(image: GrayImage, mask: GrayImage) -> Result<Self, String> {
        if image.width() < 4 || image.height() < 4 {
            return Err("模板尺寸过小".into());
        }
        if image.dimensions() != mask.dimensions() {
            return Err("模板与遮罩尺寸不一致".into());
        }
        if !mask.pixels().any(|pixel| pixel[0] >= 32) {
            return Err("模板遮罩不能全部忽略".into());
        }
        Ok(Self { image, mask })
    }

    pub fn scaled(&self, scale: f32) -> Self {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 2.0)
        } else {
            1.0
        };
        let width = ((self.image.width() as f32 * scale).round() as u32).max(4);
        let height = ((self.image.height() as f32 * scale).round() as u32).max(4);
        Self {
            image: imageops::resize(&self.image, width, height, imageops::FilterType::Triangle),
            mask: imageops::resize(&self.mask, width, height, imageops::FilterType::Nearest),
        }
    }
}

pub struct StablePresenceDetector {
    confirm_frames: u32,
    missing_frames: u32,
    hits: u32,
    misses: u32,
    present: bool,
}

impl StablePresenceDetector {
    pub const fn new(confirm_frames: u32, missing_frames: u32) -> Self {
        Self {
            confirm_frames,
            missing_frames,
            hits: 0,
            misses: 0,
            present: false,
        }
    }

    pub fn update(&mut self, matched: bool) -> bool {
        if matched {
            self.hits = self.hits.saturating_add(1);
            self.misses = 0;
            if self.hits >= self.confirm_frames.max(1) {
                self.present = true;
            }
        } else {
            self.misses = self.misses.saturating_add(1);
            self.hits = 0;
            if self.misses >= self.missing_frames.max(1) {
                self.present = false;
            }
        }
        self.present
    }

    pub fn absence_confirmed(&self) -> bool {
        !self.present && self.misses >= self.missing_frames.max(1)
    }
}

pub fn rgba_to_gray(width: u32, height: u32, rgba: &[u8]) -> Result<GrayImage, String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() < expected {
        return Err("捕获画面数据不完整".into());
    }
    let mut output = GrayImage::new(width, height);
    for (pixel, source) in output.pixels_mut().zip(rgba.chunks_exact(4)) {
        let luminance =
            (u32::from(source[0]) * 77 + u32::from(source[1]) * 150 + u32::from(source[2]) * 29)
                >> 8;
        *pixel = Luma([luminance as u8]);
    }
    Ok(output)
}

pub fn match_template(search: &GrayImage, template: &TemplateData) -> f32 {
    if search.width() < template.image.width() || search.height() < template.image.height() {
        return 0.0;
    }

    let max_x = search.width() - template.image.width();
    let max_y = search.height() - template.image.height();
    let mut best = 0.0f32;
    let mut best_position = (0u32, 0u32);
    let coarse_step = if max_x.saturating_mul(max_y) > 20_000 {
        3
    } else {
        2
    };

    let mut y = 0;
    while y <= max_y {
        let mut x = 0;
        while x <= max_x {
            let score = similarity_at(search, template, x, y, 2);
            if score > best {
                best = score;
                best_position = (x, y);
            }
            x = x.saturating_add(coarse_step);
            if x == 0 {
                break;
            }
        }
        y = y.saturating_add(coarse_step);
        if y == 0 {
            break;
        }
    }

    let start_x = best_position.0.saturating_sub(coarse_step);
    let start_y = best_position.1.saturating_sub(coarse_step);
    let end_x = best_position.0.saturating_add(coarse_step).min(max_x);
    let end_y = best_position.1.saturating_add(coarse_step).min(max_y);
    for y in start_y..=end_y {
        for x in start_x..=end_x {
            best = best.max(similarity_at(search, template, x, y, 1));
        }
    }
    best.clamp(0.0, 1.0)
}

fn similarity_at(
    search: &GrayImage,
    template: &TemplateData,
    offset_x: u32,
    offset_y: u32,
    sample_step: u32,
) -> f32 {
    let mut difference = 0u64;
    let mut weight = 0u64;
    let step = sample_step.max(1) as usize;
    for y in (0..template.image.height()).step_by(step) {
        for x in (0..template.image.width()).step_by(step) {
            let mask = u64::from(template.mask.get_pixel(x, y)[0]);
            if mask < 32 {
                continue;
            }
            let left = i32::from(search.get_pixel(offset_x + x, offset_y + y)[0]);
            let right = i32::from(template.image.get_pixel(x, y)[0]);
            difference = difference.saturating_add((left - right).unsigned_abs() as u64 * mask);
            weight = weight.saturating_add(255 * mask);
        }
    }
    if weight == 0 {
        0.0
    } else {
        1.0 - difference as f32 / weight as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> TemplateData {
        let mut image = GrayImage::from_pixel(4, 4, Luma([20]));
        image.put_pixel(1, 1, Luma([240]));
        image.put_pixel(2, 2, Luma([180]));
        TemplateData::new(image, GrayImage::from_pixel(4, 4, Luma([255]))).unwrap()
    }

    #[test]
    fn template_match_finds_embedded_icon() {
        let template = template();
        let mut search = GrayImage::from_pixel(10, 8, Luma([70]));
        imageops::replace(&mut search, &template.image, 3, 2);
        assert!(match_template(&search, &template) > 0.99);
    }

    #[test]
    fn mask_ignores_dynamic_pixel() {
        let mut template = template();
        template.mask.put_pixel(1, 1, Luma([0]));
        let mut search = template.image.clone();
        search.put_pixel(1, 1, Luma([0]));
        assert!(match_template(&search, &template) > 0.99);
    }

    #[test]
    fn stable_detector_requires_confirm_and_missing_frames() {
        let mut detector = StablePresenceDetector::new(3, 2);
        assert!(!detector.update(true));
        assert!(!detector.absence_confirmed());
        assert!(!detector.update(true));
        assert!(detector.update(true));
        assert!(detector.update(false));
        assert!(!detector.update(false));
        assert!(detector.absence_confirmed());
    }
}
