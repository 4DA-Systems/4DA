//! MUSE Image Analysis Engine
//!
//! Pure Rust extraction of creative signals from images:
//! - Dominant color palette via k-means clustering
//! - Color temperature, contrast, saturation metrics
//! - Composition analysis (symmetry, negative space, edge density)
//! - Focal point detection
//!
//! No external dependencies beyond the `image` crate.

use image::{DynamicImage, GenericImageView, Rgb};
use std::path::Path;

use crate::error::{Result, ResultExt};

use super::{ColorWeight, VisualProfile};

// ============================================================================
// Public API
// ============================================================================

/// Analyze an image file and extract a visual profile.
///
/// Resizes large images to max 512px for performance.
/// Returns color palette, composition metrics, and texture signals.
pub fn analyze_image(path: &Path) -> Result<VisualProfile> {
    let img = image::open(path)
        .with_context(|| format!("Failed to open image: {}", path.display()))?;

    // Resize for analysis (max 512px on longest side)
    let img = resize_for_analysis(&img, 512);

    let (width, height) = img.dimensions();

    // Extract color signals
    let pixels = extract_pixels(&img);
    let dominant_colors = kmeans_colors(&pixels, 5);
    let temperature = compute_temperature(&dominant_colors);
    let contrast = compute_contrast(&pixels);
    let saturation = compute_saturation(&pixels);
    let harmony = detect_harmony(&dominant_colors);

    // Extract composition signals
    let gray = img.to_luma8();
    let symmetry = compute_symmetry(&gray, width, height);
    let negative_space = compute_negative_space(&gray);
    let edge_density = compute_edge_density(&gray, width, height);
    let focal_point = detect_focal_point(&gray, width, height);
    let depth = estimate_depth(&gray, width, height);

    // Texture signals
    let grain = estimate_grain(&gray, width, height);
    let organic_score = estimate_organic_vs_geometric(&gray, width, height);

    // Convert dominant colors to hex weights
    let total_weight: f64 = dominant_colors.iter().map(|c| c.1 as f64).sum();
    let color_weights: Vec<ColorWeight> = dominant_colors
        .iter()
        .map(|(rgb, count)| ColorWeight {
            hex: format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
            weight: *count as f64 / total_weight,
        })
        .collect();

    Ok(VisualProfile {
        dominant_colors: color_weights,
        temperature,
        contrast,
        saturation,
        harmony: Some(harmony),
        symmetry,
        negative_space,
        focal_point: Some(focal_point),
        depth,
        grain,
        organic_vs_geometric: organic_score,
    })
}

// ============================================================================
// Color Analysis
// ============================================================================

/// Extract RGB pixels from image (sampled for performance)
fn extract_pixels(img: &DynamicImage) -> Vec<[u8; 3]> {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let total = w * h;

    // Sample at most 10K pixels for speed
    let step = (total / 10_000).max(1);

    rgb.pixels()
        .step_by(step)
        .map(|Rgb(p)| *p)
        .collect()
}

/// K-means clustering for dominant colors
fn kmeans_colors(pixels: &[[u8; 3]], k: usize) -> Vec<([u8; 3], u32)> {
    if pixels.is_empty() {
        return vec![([128, 128, 128], 1)];
    }

    // Initialize centroids by sampling evenly across pixel array
    let mut centroids: Vec<[f64; 3]> = (0..k)
        .map(|i| {
            let idx = i * pixels.len() / k;
            let p = pixels[idx.min(pixels.len() - 1)];
            [p[0] as f64, p[1] as f64, p[2] as f64]
        })
        .collect();

    let mut assignments = vec![0usize; pixels.len()];

    // 15 iterations is sufficient for color clustering
    for _ in 0..15 {
        // Assign each pixel to nearest centroid
        for (i, px) in pixels.iter().enumerate() {
            let mut best_dist = f64::MAX;
            let mut best_k = 0;
            for (j, c) in centroids.iter().enumerate() {
                let dist = color_distance_sq(px, c);
                if dist < best_dist {
                    best_dist = dist;
                    best_k = j;
                }
            }
            assignments[i] = best_k;
        }

        // Update centroids
        let mut sums = vec![[0.0f64; 3]; k];
        let mut counts = vec![0u32; k];

        for (i, px) in pixels.iter().enumerate() {
            let c = assignments[i];
            sums[c][0] += px[0] as f64;
            sums[c][1] += px[1] as f64;
            sums[c][2] += px[2] as f64;
            counts[c] += 1;
        }

        for j in 0..k {
            if counts[j] > 0 {
                centroids[j][0] = sums[j][0] / counts[j] as f64;
                centroids[j][1] = sums[j][1] / counts[j] as f64;
                centroids[j][2] = sums[j][2] / counts[j] as f64;
            }
        }
    }

    // Collect results, sorted by cluster size (dominant first)
    let mut counts = vec![0u32; k];
    for &a in &assignments {
        counts[a] += 1;
    }

    let mut result: Vec<([u8; 3], u32)> = centroids
        .iter()
        .zip(counts.iter())
        .filter(|(_, &count)| count > 0)
        .map(|(c, &count)| {
            ([c[0] as u8, c[1] as u8, c[2] as u8], count)
        })
        .collect();

    result.sort_by(|a, b| b.1.cmp(&a.1));
    result
}

fn color_distance_sq(px: &[u8; 3], centroid: &[f64; 3]) -> f64 {
    let dr = px[0] as f64 - centroid[0];
    let dg = px[1] as f64 - centroid[1];
    let db = px[2] as f64 - centroid[2];
    dr * dr + dg * dg + db * db
}

/// Compute color temperature (0=cool, 1=warm) from dominant colors
fn compute_temperature(colors: &[([u8; 3], u32)]) -> f64 {
    if colors.is_empty() {
        return 0.5;
    }
    let total_weight: f64 = colors.iter().map(|c| c.1 as f64).sum();
    let mut warm_score = 0.0;

    for (rgb, count) in colors {
        let weight = *count as f64 / total_weight;
        // Warm = more red/yellow, Cool = more blue
        let r = rgb[0] as f64 / 255.0;
        let b = rgb[2] as f64 / 255.0;
        warm_score += (r - b + 1.0) / 2.0 * weight; // Normalize to 0-1
    }

    warm_score.clamp(0.0, 1.0)
}

/// Compute contrast (luminance range) from pixel data
fn compute_contrast(pixels: &[[u8; 3]]) -> f64 {
    if pixels.is_empty() {
        return 0.5;
    }

    let luminances: Vec<f64> = pixels
        .iter()
        .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
        .collect();

    let min = luminances.iter().cloned().fold(f64::MAX, f64::min);
    let max = luminances.iter().cloned().fold(f64::MIN, f64::max);

    // Also compute standard deviation for a more nuanced measure
    let mean: f64 = luminances.iter().sum::<f64>() / luminances.len() as f64;
    let variance: f64 = luminances.iter().map(|l| (l - mean).powi(2)).sum::<f64>()
        / luminances.len() as f64;
    let std_dev = variance.sqrt();

    // Blend range-based and std-dev-based contrast
    let range_contrast = (max - min) / 255.0;
    let std_contrast = (std_dev / 128.0).min(1.0); // Normalize

    (range_contrast * 0.4 + std_contrast * 0.6).clamp(0.0, 1.0)
}

/// Average saturation across pixels (HSL-based)
fn compute_saturation(pixels: &[[u8; 3]]) -> f64 {
    if pixels.is_empty() {
        return 0.5;
    }

    let total_sat: f64 = pixels
        .iter()
        .map(|p| {
            let r = p[0] as f64 / 255.0;
            let g = p[1] as f64 / 255.0;
            let b = p[2] as f64 / 255.0;
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let l = (max + min) / 2.0;
            if (max - min).abs() < f64::EPSILON {
                0.0
            } else if l <= 0.5 {
                (max - min) / (max + min)
            } else {
                (max - min) / (2.0 - max - min)
            }
        })
        .sum();

    (total_sat / pixels.len() as f64).clamp(0.0, 1.0)
}

/// Detect palette harmony type
fn detect_harmony(colors: &[([u8; 3], u32)]) -> String {
    if colors.len() < 2 {
        return "monochromatic".to_string();
    }

    // Convert to hue angles
    let hues: Vec<f64> = colors
        .iter()
        .map(|(rgb, _)| rgb_to_hue(rgb[0], rgb[1], rgb[2]))
        .collect();

    // Check saturation — if all low, it's monochromatic
    let avg_sat: f64 = colors
        .iter()
        .map(|(rgb, _)| {
            let max = rgb[0].max(rgb[1]).max(rgb[2]) as f64;
            let min = rgb[0].min(rgb[1]).min(rgb[2]) as f64;
            if max < f64::EPSILON { 0.0 } else { (max - min) / max }
        })
        .sum::<f64>() / colors.len() as f64;

    if avg_sat < 0.15 {
        return "monochromatic".to_string();
    }

    // Check hue spread
    let mut max_gap = 0.0f64;
    for i in 0..hues.len() {
        for j in (i + 1)..hues.len() {
            let diff = hue_distance(hues[i], hues[j]);
            max_gap = max_gap.max(diff);
        }
    }

    if max_gap < 30.0 {
        "analogous".to_string()
    } else if (max_gap - 180.0).abs() < 30.0 {
        "complementary".to_string()
    } else if max_gap > 100.0 && max_gap < 150.0 {
        "split_complementary".to_string()
    } else if max_gap > 100.0 {
        "triadic".to_string()
    } else {
        "mixed".to_string()
    }
}

fn rgb_to_hue(r: u8, g: u8, b: u8) -> f64 {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    if delta < f64::EPSILON {
        return 0.0;
    }

    let hue = if (max - r).abs() < f64::EPSILON {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < f64::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    if hue < 0.0 { hue + 360.0 } else { hue }
}

fn hue_distance(a: f64, b: f64) -> f64 {
    let diff = (a - b).abs();
    diff.min(360.0 - diff)
}

// ============================================================================
// Composition Analysis
// ============================================================================

/// Bilateral symmetry score (0=asymmetric, 1=symmetric)
fn compute_symmetry(gray: &image::GrayImage, width: u32, height: u32) -> f64 {
    let half_w = width / 2;
    let mut match_count = 0u64;
    let mut total = 0u64;
    let threshold = 20u8; // Allow small luminance differences

    // Sample rows for speed
    let row_step = (height / 100).max(1);

    for y in (0..height).step_by(row_step as usize) {
        for x in 0..half_w {
            let mirror_x = width - 1 - x;
            let left = gray.get_pixel(x, y).0[0];
            let right = gray.get_pixel(mirror_x, y).0[0];
            if left.abs_diff(right) <= threshold {
                match_count += 1;
            }
            total += 1;
        }
    }

    if total == 0 {
        return 0.5;
    }
    (match_count as f64 / total as f64).clamp(0.0, 1.0)
}

/// Negative space ratio (0=dense, 1=spacious)
fn compute_negative_space(gray: &image::GrayImage) -> f64 {
    // Use Otsu-style thresholding to separate foreground/background
    let histogram = compute_histogram(gray);
    let threshold = otsu_threshold(&histogram);

    let total_pixels = gray.width() as u64 * gray.height() as u64;
    let background_pixels: u64 = gray
        .pixels()
        .filter(|p| p.0[0] > threshold)
        .count() as u64;

    (background_pixels as f64 / total_pixels as f64).clamp(0.0, 1.0)
}

/// Edge density via Sobel filter (0=minimal, 1=busy)
fn compute_edge_density(gray: &image::GrayImage, width: u32, height: u32) -> f64 {
    if width < 3 || height < 3 {
        return 0.5;
    }

    let mut edge_sum = 0.0f64;
    let mut count = 0u64;
    let edge_threshold = 50.0;

    // Sample for performance
    let step = ((width * height) as usize / 5000).max(1);
    let mut idx = 0;

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            idx += 1;
            if idx % step != 0 {
                continue;
            }

            // Sobel X and Y
            let gx = sobel_x(gray, x, y);
            let gy = sobel_y(gray, x, y);
            let magnitude = ((gx * gx + gy * gy) as f64).sqrt();

            if magnitude > edge_threshold {
                edge_sum += 1.0;
            }
            count += 1;
        }
    }

    if count == 0 {
        return 0.5;
    }
    (edge_sum / count as f64).clamp(0.0, 1.0)
}

fn sobel_x(gray: &image::GrayImage, x: u32, y: u32) -> i32 {
    let p = |dx: i32, dy: i32| -> i32 {
        gray.get_pixel((x as i32 + dx) as u32, (y as i32 + dy) as u32).0[0] as i32
    };
    -p(-1, -1) + p(1, -1) - 2 * p(-1, 0) + 2 * p(1, 0) - p(-1, 1) + p(1, 1)
}

fn sobel_y(gray: &image::GrayImage, x: u32, y: u32) -> i32 {
    let p = |dx: i32, dy: i32| -> i32 {
        gray.get_pixel((x as i32 + dx) as u32, (y as i32 + dy) as u32).0[0] as i32
    };
    -p(-1, -1) - 2 * p(0, -1) - p(1, -1) + p(-1, 1) + 2 * p(0, 1) + p(1, 1)
}

/// Detect dominant focal point position
fn detect_focal_point(gray: &image::GrayImage, width: u32, height: u32) -> String {
    if width < 3 || height < 3 {
        return "center".to_string();
    }

    // Compute edge magnitude in a grid of 9 cells (3x3)
    let cell_w = width / 3;
    let cell_h = height / 3;
    let mut cell_energy = [[0.0f64; 3]; 3];

    let step = ((width * height) as usize / 3000).max(1);
    let mut idx = 0;

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            idx += 1;
            if idx % step != 0 {
                continue;
            }
            let gx = sobel_x(gray, x, y) as f64;
            let gy = sobel_y(gray, x, y) as f64;
            let mag = (gx * gx + gy * gy).sqrt();

            let cx = ((x / cell_w) as usize).min(2);
            let cy = ((y / cell_h) as usize).min(2);
            cell_energy[cy][cx] += mag;
        }
    }

    // Find the cell with maximum energy
    let mut max_energy = 0.0f64;
    let mut max_cx = 1;
    let mut max_cy = 1;

    for cy in 0..3 {
        for cx in 0..3 {
            if cell_energy[cy][cx] > max_energy {
                max_energy = cell_energy[cy][cx];
                max_cx = cx;
                max_cy = cy;
            }
        }
    }

    // Map grid position to focal point name
    match (max_cx, max_cy) {
        (1, 1) => "center".to_string(),
        (0, 0) | (2, 0) | (0, 2) | (2, 2) => "rule_of_thirds".to_string(),
        (0, _) | (2, _) => "edge".to_string(),
        (_, 0) | (_, 2) => "edge".to_string(),
        _ => "distributed".to_string(),
    }
}

/// Estimate perceived depth (0=flat, 1=deep)
fn estimate_depth(gray: &image::GrayImage, width: u32, height: u32) -> f64 {
    if height < 10 {
        return 0.5;
    }

    // Compare blur/detail between top and bottom halves
    // Images with depth tend to have more detail in foreground (bottom)
    let mid_y = height / 2;

    let top_variance = region_variance(gray, 0, 0, width, mid_y);
    let bottom_variance = region_variance(gray, 0, mid_y, width, height);

    // If bottom has more variance than top, suggests depth
    let ratio = if top_variance > f64::EPSILON {
        (bottom_variance / top_variance).min(3.0) / 3.0
    } else {
        0.5
    };

    ratio.clamp(0.0, 1.0)
}

/// Estimate grain/noise level (0=clean, 1=grainy)
fn estimate_grain(gray: &image::GrayImage, width: u32, height: u32) -> f64 {
    if width < 4 || height < 4 {
        return 0.0;
    }

    // Measure high-frequency variation (adjacent pixel differences)
    let mut diff_sum = 0.0f64;
    let mut count = 0u64;
    let step = ((width * height) as usize / 5000).max(1);
    let mut idx = 0;

    for y in 0..(height - 1) {
        for x in 0..(width - 1) {
            idx += 1;
            if idx % step != 0 {
                continue;
            }
            let p = gray.get_pixel(x, y).0[0] as f64;
            let px_right = gray.get_pixel(x + 1, y).0[0] as f64;
            let px_down = gray.get_pixel(x, y + 1).0[0] as f64;
            diff_sum += (p - px_right).abs() + (p - px_down).abs();
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }

    // Normalize: typical grain produces avg diff ~5-15
    let avg_diff = diff_sum / count as f64;
    (avg_diff / 30.0).clamp(0.0, 1.0)
}

/// Estimate organic vs geometric (0=geometric, 1=organic)
fn estimate_organic_vs_geometric(gray: &image::GrayImage, width: u32, height: u32) -> f64 {
    if width < 3 || height < 3 {
        return 0.5;
    }

    // Organic forms have varied edge directions
    // Geometric forms have consistent edge directions (horizontal/vertical dominance)
    let mut h_edges = 0.0f64;
    let mut v_edges = 0.0f64;
    let mut diag_edges = 0.0f64;
    let step = ((width * height) as usize / 3000).max(1);
    let mut idx = 0;

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            idx += 1;
            if idx % step != 0 {
                continue;
            }
            let gx = sobel_x(gray, x, y).abs() as f64;
            let gy = sobel_y(gray, x, y).abs() as f64;

            if gx + gy < 30.0 {
                continue; // Skip low-energy pixels
            }

            if gx > gy * 2.0 {
                h_edges += 1.0;
            } else if gy > gx * 2.0 {
                v_edges += 1.0;
            } else {
                diag_edges += 1.0;
            }
        }
    }

    let total = h_edges + v_edges + diag_edges;
    if total < f64::EPSILON {
        return 0.5;
    }

    // High diagonal ratio = organic, high h/v ratio = geometric
    let geometric_ratio = (h_edges + v_edges) / total;
    (1.0 - geometric_ratio).clamp(0.0, 1.0)
}

// ============================================================================
// Helpers
// ============================================================================

fn resize_for_analysis(img: &DynamicImage, max_dim: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w <= max_dim && h <= max_dim {
        return img.clone();
    }
    let scale = max_dim as f64 / w.max(h) as f64;
    let new_w = (w as f64 * scale) as u32;
    let new_h = (h as f64 * scale) as u32;
    img.resize(new_w, new_h, image::imageops::FilterType::Triangle)
}

fn compute_histogram(gray: &image::GrayImage) -> [u32; 256] {
    let mut hist = [0u32; 256];
    for p in gray.pixels() {
        hist[p.0[0] as usize] += 1;
    }
    hist
}

/// Otsu's method for automatic thresholding
fn otsu_threshold(histogram: &[u32; 256]) -> u8 {
    let total: u32 = histogram.iter().sum();
    if total == 0 {
        return 128;
    }

    let mut sum_total: f64 = 0.0;
    for (i, &count) in histogram.iter().enumerate() {
        sum_total += i as f64 * count as f64;
    }

    let mut sum_bg = 0.0f64;
    let mut weight_bg = 0u32;
    let mut max_variance = 0.0f64;
    let mut best_threshold = 0u8;

    for (t, &count) in histogram.iter().enumerate() {
        weight_bg += count;
        if weight_bg == 0 {
            continue;
        }
        let weight_fg = total - weight_bg;
        if weight_fg == 0 {
            break;
        }

        sum_bg += t as f64 * count as f64;
        let mean_bg = sum_bg / weight_bg as f64;
        let mean_fg = (sum_total - sum_bg) / weight_fg as f64;

        let between_variance =
            weight_bg as f64 * weight_fg as f64 * (mean_bg - mean_fg).powi(2);

        if between_variance > max_variance {
            max_variance = between_variance;
            best_threshold = t as u8;
        }
    }

    best_threshold
}

fn region_variance(
    gray: &image::GrayImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
) -> f64 {
    let mut sum = 0.0f64;
    let mut sq_sum = 0.0f64;
    let mut count = 0u64;
    let step = (((x1 - x0) * (y1 - y0)) as usize / 2000).max(1);
    let mut idx = 0;

    for y in y0..y1 {
        for x in x0..x1 {
            idx += 1;
            if idx % step != 0 {
                continue;
            }
            let v = gray.get_pixel(x, y).0[0] as f64;
            sum += v;
            sq_sum += v * v;
            count += 1;
        }
    }

    if count < 2 {
        return 0.0;
    }

    let mean = sum / count as f64;
    (sq_sum / count as f64) - mean * mean
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage};

    fn make_test_image(r: u8, g: u8, b: u8, w: u32, h: u32) -> DynamicImage {
        let img = RgbImage::from_fn(w, h, |_, _| Rgb([r, g, b]));
        DynamicImage::ImageRgb8(img)
    }

    fn make_gradient_image(w: u32, h: u32) -> DynamicImage {
        let img = RgbImage::from_fn(w, h, |x, _| {
            let v = (x as f64 / w as f64 * 255.0) as u8;
            Rgb([v, v, v])
        });
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_solid_color_analysis() {
        let img = make_test_image(200, 100, 50, 100, 100);
        let pixels = extract_pixels(&img);
        let colors = kmeans_colors(&pixels, 3);

        // Solid image should produce one dominant cluster
        assert!(!colors.is_empty());
        // The dominant color should be close to (200, 100, 50)
        let (dominant, _) = &colors[0];
        assert!((dominant[0] as i32 - 200).abs() < 10);
    }

    #[test]
    fn test_temperature_warm() {
        // Red/warm image
        let colors = vec![([200u8, 80, 50], 100)];
        let temp = compute_temperature(&colors);
        assert!(temp > 0.6, "Warm colors should have high temperature: {temp}");
    }

    #[test]
    fn test_temperature_cool() {
        // Blue/cool image
        let colors = vec![([50u8, 80, 200], 100)];
        let temp = compute_temperature(&colors);
        assert!(temp < 0.4, "Cool colors should have low temperature: {temp}");
    }

    #[test]
    fn test_contrast_flat() {
        let img = make_test_image(128, 128, 128, 100, 100);
        let pixels = extract_pixels(&img);
        let contrast = compute_contrast(&pixels);
        assert!(contrast < 0.1, "Flat image should have low contrast: {contrast}");
    }

    #[test]
    fn test_contrast_high() {
        // Gradient has high contrast
        let img = make_gradient_image(100, 100);
        let pixels = extract_pixels(&img);
        let contrast = compute_contrast(&pixels);
        assert!(contrast > 0.3, "Gradient should have measurable contrast: {contrast}");
    }

    #[test]
    fn test_symmetry_symmetric() {
        // Symmetric gradient (left-to-right mirrored)
        let img = RgbImage::from_fn(100, 100, |x, _| {
            let v = if x < 50 { x * 5 } else { (99 - x) * 5 } as u8;
            Rgb([v, v, v])
        });
        let gray = DynamicImage::ImageRgb8(img).to_luma8();
        let sym = compute_symmetry(&gray, 100, 100);
        assert!(sym > 0.7, "Symmetric image should score high: {sym}");
    }

    #[test]
    fn test_edge_density_smooth() {
        let img = make_test_image(128, 128, 128, 100, 100);
        let gray = img.to_luma8();
        let density = compute_edge_density(&gray, 100, 100);
        assert!(density < 0.1, "Solid image should have low edge density: {density}");
    }

    #[test]
    fn test_harmony_monochromatic() {
        let colors = vec![([100, 100, 100], 50), ([120, 120, 120], 30)];
        let harmony = detect_harmony(&colors);
        assert_eq!(harmony, "monochromatic");
    }

    #[test]
    fn test_otsu_bimodal() {
        // Bimodal histogram: half dark, half light
        let mut hist = [0u32; 256];
        for i in 0..50 { hist[i] = 100; }
        for i in 200..256 { hist[i] = 100; }
        let threshold = otsu_threshold(&hist);
        // Threshold should be between the two modes
        assert!(threshold > 40 && threshold < 210, "Otsu threshold: {threshold}");
    }

    #[test]
    fn test_kmeans_two_colors() {
        let mut pixels = Vec::new();
        for _ in 0..500 { pixels.push([255, 0, 0]); }
        for _ in 0..500 { pixels.push([0, 0, 255]); }

        let colors = kmeans_colors(&pixels, 2);
        assert_eq!(colors.len(), 2);
        // Both clusters should have ~500 pixels each
        assert!(colors[0].1 > 400);
        assert!(colors[1].1 > 400);
    }
}
