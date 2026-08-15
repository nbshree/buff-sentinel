use image::{GrayImage, imageops};

#[cfg(test)]
use image::Luma;

#[derive(Clone)]
pub struct TemplateData {
    pub image: GrayImage,
    pub mask: GrayImage,
    coarse_samples: TemplateSamples,
    fine_samples: TemplateSamples,
}

#[derive(Clone)]
struct TemplateSamples {
    pixels: Vec<TemplatePixel>,
    total_weight: u64,
}

#[derive(Clone, Copy)]
struct TemplatePixel {
    x: u32,
    y: u32,
    value: u8,
    mask: u8,
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
        let coarse_samples = TemplateSamples::new(&image, &mask, 2);
        let fine_samples = TemplateSamples::new(&image, &mask, 1);
        Ok(Self {
            image,
            mask,
            coarse_samples,
            fine_samples,
        })
    }

    pub fn scaled(&self, scale: f32) -> Self {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 2.0)
        } else {
            1.0
        };
        let width = ((self.image.width() as f32 * scale).round() as u32).max(4);
        let height = ((self.image.height() as f32 * scale).round() as u32).max(4);
        Self::new(
            imageops::resize(&self.image, width, height, imageops::FilterType::Triangle),
            imageops::resize(&self.mask, width, height, imageops::FilterType::Nearest),
        )
        .expect("scaled templates preserve valid dimensions and masks")
    }
}

impl TemplateSamples {
    fn new(image: &GrayImage, mask: &GrayImage, sample_step: u32) -> Self {
        let mut pixels = Vec::new();
        let mut total_weight = 0u64;
        let step = sample_step.max(1) as usize;
        for y in (0..image.height()).step_by(step) {
            for x in (0..image.width()).step_by(step) {
                let mask = mask.get_pixel(x, y)[0];
                if mask < 32 {
                    continue;
                }
                pixels.push(TemplatePixel {
                    x,
                    y,
                    value: image.get_pixel(x, y)[0],
                    mask,
                });
                total_weight = total_weight.saturating_add(255 * u64::from(mask));
            }
        }
        Self {
            pixels,
            total_weight,
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

pub fn rgba_to_gray_with_buffer(
    width: u32,
    height: u32,
    rgba: &[u8],
    mut output: Vec<u8>,
) -> Result<GrayImage, String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() < expected {
        return Err("捕获画面数据不完整".into());
    }
    let pixel_count = width as usize * height as usize;
    output.resize(pixel_count, 0);
    for (pixel, source) in output.iter_mut().zip(rgba.chunks_exact(4)) {
        let luminance =
            (u32::from(source[0]) * 77 + u32::from(source[1]) * 150 + u32::from(source[2]) * 29)
                >> 8;
        *pixel = luminance as u8;
    }
    GrayImage::from_raw(width, height, output)
        .ok_or_else(|| "无法创建 Buff 灰度识别画面".to_string())
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
            let score = similarity_at(search, &template.coarse_samples, x, y, best);
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
            best = best.max(similarity_at(search, &template.fine_samples, x, y, best));
        }
    }
    best.clamp(0.0, 1.0)
}

fn similarity_at(
    search: &GrayImage,
    samples: &TemplateSamples,
    offset_x: u32,
    offset_y: u32,
    minimum_score: f32,
) -> f32 {
    if samples.total_weight == 0 {
        return 0.0;
    }
    let mut difference = 0u64;
    let maximum_difference =
        ((1.0 - f64::from(minimum_score)) * samples.total_weight as f64 + 1.0).max(0.0);
    let search_width = search.width() as usize;
    let search_pixels = search.as_raw();
    for pixel in &samples.pixels {
        let index = (offset_y + pixel.y) as usize * search_width + (offset_x + pixel.x) as usize;
        let left = i32::from(search_pixels[index]);
        let right = i32::from(pixel.value);
        difference =
            difference.saturating_add((left - right).unsigned_abs() as u64 * u64::from(pixel.mask));
        if difference as f64 > maximum_difference {
            return 0.0;
        }
    }
    1.0 - difference as f32 / samples.total_weight as f32
}

#[cfg(test)]
mod tests {
    use std::{hint::black_box, time::Instant};

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
        let image = template().image;
        let mut mask = GrayImage::from_pixel(4, 4, Luma([255]));
        mask.put_pixel(1, 1, Luma([0]));
        let template = TemplateData::new(image, mask).unwrap();
        let mut search = template.image.clone();
        search.put_pixel(1, 1, Luma([0]));
        assert!(match_template(&search, &template) > 0.99);
    }

    #[test]
    fn optimized_matcher_preserves_legacy_confidence() {
        for seed in 0..8 {
            let template = patterned_template(24, 24, seed);
            let mut search = patterned_image(180, 96, seed.wrapping_add(17));
            if seed % 2 == 0 {
                imageops::replace(
                    &mut search,
                    &template.image,
                    13 + i64::from(seed),
                    21 + i64::from(seed),
                );
            }

            let optimized = match_template(&search, &template);
            let legacy = legacy_match_template(&search, &template);

            assert!(
                (optimized - legacy).abs() <= f32::EPSILON * 4.0,
                "seed {seed}: optimized={optimized}, legacy={legacy}"
            );
        }
    }

    #[test]
    fn grayscale_conversion_reuses_the_supplied_allocation() {
        let rgba = vec![120; 64 * 32 * 4];
        let buffer = Vec::with_capacity(64 * 32);
        let capacity = buffer.capacity();

        let gray = rgba_to_gray_with_buffer(64, 32, &rgba, buffer).unwrap();

        assert_eq!(gray.dimensions(), (64, 32));
        assert_eq!(gray.as_raw().capacity(), capacity);
    }

    #[test]
    #[ignore = "manual release-mode performance benchmark"]
    fn benchmark_unmatched_template_counts() {
        let template = patterned_template(32, 32, 3);
        let search = patterned_image(240, 120, 91);

        for listener_count in [1usize, 4, 8] {
            let iterations = 8;
            let legacy_started = Instant::now();
            for _ in 0..iterations {
                for _ in 0..listener_count {
                    black_box(legacy_match_template(
                        black_box(&search),
                        black_box(&template),
                    ));
                }
            }
            let legacy_elapsed = legacy_started.elapsed();

            let optimized_started = Instant::now();
            for _ in 0..iterations {
                for _ in 0..listener_count {
                    black_box(match_template(black_box(&search), black_box(&template)));
                }
            }
            let optimized_elapsed = optimized_started.elapsed();
            let ratio = optimized_elapsed.as_secs_f64() / legacy_elapsed.as_secs_f64();
            eprintln!(
                "listeners={listener_count}, legacy={legacy_elapsed:?}, optimized={optimized_elapsed:?}, ratio={ratio:.3}"
            );
            assert!(
                ratio <= 0.75,
                "optimized matcher must be at least 25% faster for {listener_count} listeners"
            );
        }
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

    fn patterned_template(width: u32, height: u32, seed: u8) -> TemplateData {
        let image = patterned_image(width, height, seed);
        let mut mask = GrayImage::from_pixel(width, height, Luma([255]));
        for y in 0..height {
            for x in 0..width {
                if (x + y + u32::from(seed)).is_multiple_of(7) {
                    mask.put_pixel(x, y, Luma([0]));
                }
            }
        }
        TemplateData::new(image, mask).unwrap()
    }

    fn patterned_image(width: u32, height: u32, seed: u8) -> GrayImage {
        GrayImage::from_fn(width, height, |x, y| {
            Luma([((x * 31 + y * 17 + u32::from(seed) * 13) % 251) as u8])
        })
    }

    fn legacy_match_template(search: &GrayImage, template: &TemplateData) -> f32 {
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
                let score = legacy_similarity_at(search, template, x, y, 2);
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
                best = best.max(legacy_similarity_at(search, template, x, y, 1));
            }
        }
        best.clamp(0.0, 1.0)
    }

    fn legacy_similarity_at(
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
}
