//! High-performance template matching with alpha transparency masking,
//! multi-stage spatial grid anchor rejection, and Rayon parallelization in Rust.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use image::RgbaImage;
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub matched: bool,
    pub confidence: f32,
    pub box_x: u32,
    pub box_y: u32,
    pub width: u32,
    pub height: u32,
    pub center_x: u32,
    pub center_y: u32,
    pub template_path: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ActivePixel {
    pub offset_x: u32,
    pub offset_y: u32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub alpha: u8,
}

#[derive(Debug, Clone)]
pub struct CachedTemplate {
    pub image: RgbaImage,
    pub has_alpha: bool,
    pub is_all_opaque: bool,
    pub width: u32,
    pub height: u32,
    pub active_pixels: Vec<ActivePixel>,
    pub primary_anchors: Vec<ActivePixel>,
    pub secondary_anchors: Vec<ActivePixel>,
    pub samples: Vec<ActivePixel>,
    pub max_total_diff: u32,
    pub primary_max_diff: u32,
    pub secondary_max_diff: u32,
    pub sample_max_diff: u32,
}

pub struct TemplateMatcher {
    asset_root: PathBuf,
    cache: HashMap<String, Option<CachedTemplate>>,
}

impl TemplateMatcher {
    pub fn new<P: AsRef<Path>>(asset_root: P) -> Self {
        Self {
            asset_root: asset_root.as_ref().to_path_buf(),
            cache: HashMap::new(),
        }
    }

    pub fn load_template(&mut self, relative_path: &str) -> Option<&CachedTemplate> {
        if !self.cache.contains_key(relative_path) {
            let full_path = self.asset_root.join(relative_path);
            let target_path = if full_path.exists() {
                full_path
            } else {
                PathBuf::from(relative_path)
            };

            let cached = if let Ok(img) = image::open(&target_path) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let mut has_alpha = false;
                let mut is_all_opaque = true;
                let mut active_pixels = Vec::with_capacity((w * h) as usize);
                let mut max_total_diff: u32 = 0;

                for y in 0..h {
                    for x in 0..w {
                        let p = rgba.get_pixel(x, y);
                        let alpha = p[3];
                        if alpha < 255 {
                            has_alpha = true;
                            if alpha > 64 {
                                is_all_opaque = false;
                            }
                        }

                        if alpha > 64 {
                            active_pixels.push(ActivePixel {
                                offset_x: x,
                                offset_y: y,
                                r: p[0],
                                g: p[1],
                                b: p[2],
                                alpha,
                            });
                            max_total_diff += (765 * alpha as u32) / 255;
                        }
                    }
                }

                if active_pixels.is_empty() {
                    for y in 0..h {
                        for x in 0..w {
                            let p = rgba.get_pixel(x, y);
                            active_pixels.push(ActivePixel {
                                offset_x: x,
                                offset_y: y,
                                r: p[0],
                                g: p[1],
                                b: p[2],
                                alpha: 255,
                            });
                            max_total_diff += 765;
                        }
                    }
                    is_all_opaque = true;
                }

                // Primary anchors: 8 points on a 3x3 grid (excluding center or covering perimeter)
                let mut primary_anchors = Vec::with_capacity(8);
                let mut primary_max_diff: u32 = 0;
                let p_cols = 3;
                let p_rows = 3;
                let pw = (w as f32 / p_cols as f32).max(1.0);
                let ph = (h as f32 / p_rows as f32).max(1.0);

                for gy in 0..p_rows {
                    for gx in 0..p_cols {
                        let target_x = (gx as f32 + 0.5) * pw;
                        let target_y = (gy as f32 + 0.5) * ph;

                        if let Some(best_p) = active_pixels.iter().min_by_key(|p| {
                            let dx = p.offset_x as f32 - target_x;
                            let dy = p.offset_y as f32 - target_y;
                            (dx * dx + dy * dy) as u32
                        }) {
                            if !primary_anchors.iter().any(|a: &ActivePixel| a.offset_x == best_p.offset_x && a.offset_y == best_p.offset_y) {
                                primary_anchors.push(*best_p);
                                primary_max_diff += (765 * best_p.alpha as u32) / 255;
                            }
                        }
                    }
                }

                // Secondary anchors: 16 points on a 4x4 grid
                let mut secondary_anchors = Vec::with_capacity(16);
                let mut secondary_max_diff: u32 = 0;
                let s_cols = 4;
                let s_rows = 4;
                let sw = (w as f32 / s_cols as f32).max(1.0);
                let sh = (h as f32 / s_rows as f32).max(1.0);

                for gy in 0..s_rows {
                    for gx in 0..s_cols {
                        let target_x = (gx as f32 + 0.5) * sw;
                        let target_y = (gy as f32 + 0.5) * sh;

                        if let Some(best_p) = active_pixels.iter().min_by_key(|p| {
                            let dx = p.offset_x as f32 - target_x;
                            let dy = p.offset_y as f32 - target_y;
                            (dx * dx + dy * dy) as u32
                        }) {
                            if !secondary_anchors.iter().any(|a: &ActivePixel| a.offset_x == best_p.offset_x && a.offset_y == best_p.offset_y) {
                                secondary_anchors.push(*best_p);
                                secondary_max_diff += (765 * best_p.alpha as u32) / 255;
                            }
                        }
                    }
                }

                // Intermediate samples (up to 49) on a 7x7 grid
                let mut samples = Vec::with_capacity(49);
                let mut sample_max_diff: u32 = 0;
                let sc_cols = 7;
                let sc_rows = 7;
                let sc_w = (w as f32 / sc_cols as f32).max(1.0);
                let sc_h = (h as f32 / sc_rows as f32).max(1.0);

                for gy in 0..sc_rows {
                    for gx in 0..sc_cols {
                        let target_x = (gx as f32 + 0.5) * sc_w;
                        let target_y = (gy as f32 + 0.5) * sc_h;

                        if let Some(best_p) = active_pixels.iter().min_by_key(|p| {
                            let dx = p.offset_x as f32 - target_x;
                            let dy = p.offset_y as f32 - target_y;
                            (dx * dx + dy * dy) as u32
                        }) {
                            if !samples.iter().any(|s: &ActivePixel| s.offset_x == best_p.offset_x && s.offset_y == best_p.offset_y) {
                                samples.push(*best_p);
                                sample_max_diff += (765 * best_p.alpha as u32) / 255;
                            }
                        }
                    }
                }

                Some(CachedTemplate {
                    image: rgba,
                    has_alpha,
                    is_all_opaque,
                    width: w,
                    height: h,
                    active_pixels,
                    primary_anchors,
                    secondary_anchors,
                    samples,
                    max_total_diff: max_total_diff.max(1),
                    primary_max_diff: primary_max_diff.max(1),
                    secondary_max_diff: secondary_max_diff.max(1),
                    sample_max_diff: sample_max_diff.max(1),
                })
            } else {
                None
            };

            self.cache.insert(relative_path.to_string(), cached);
        }

        self.cache.get(relative_path).and_then(|opt| opt.as_ref())
    }

    pub fn find_match(
        &mut self,
        screen: &RgbaImage,
        template_path: &str,
        threshold: f32,
        roi: Option<(u32, u32, u32, u32)>, // (x1, y1, x2, y2)
    ) -> MatchResult {
        let tmpl = match self.load_template(template_path) {
            Some(t) => t.clone(),
            None => {
                return MatchResult {
                    matched: false,
                    confidence: 0.0,
                    box_x: 0,
                    box_y: 0,
                    width: 0,
                    height: 0,
                    center_x: 0,
                    center_y: 0,
                    template_path: template_path.to_string(),
                };
            }
        };

        let (screen_w, screen_h) = screen.dimensions();
        let tw = tmpl.width;
        let th = tmpl.height;

        let (roi_x1, roi_y1, roi_x2, roi_y2) = match roi {
            Some((x1, y1, x2, y2)) => (
                x1.min(screen_w),
                y1.min(screen_h),
                x2.min(screen_w).max(x1 + 1),
                y2.min(screen_h).max(y1 + 1),
            ),
            None => (0, 0, screen_w, screen_h),
        };

        if (roi_x2 - roi_x1) < tw || (roi_y2 - roi_y1) < th {
            return MatchResult {
                matched: false,
                confidence: 0.0,
                box_x: 0,
                box_y: 0,
                width: tw,
                height: th,
                center_x: 0,
                center_y: 0,
                template_path: template_path.to_string(),
            };
        }

        let max_x = roi_x2 - tw;
        let max_y = roi_y2 - th;

        let screen_raw = screen.as_raw();
        let screen_stride = (screen_w * 4) as usize;

        // Precompute byte offsets for anchors, samples, and active pixels
        struct PreparedPixel {
            byte_offset: usize,
            r: u8,
            g: u8,
            b: u8,
            alpha: u8,
        }

        let prep_primary: Vec<PreparedPixel> = tmpl.primary_anchors.iter().map(|p| PreparedPixel {
            byte_offset: (p.offset_y as usize) * screen_stride + (p.offset_x as usize) * 4,
            r: p.r,
            g: p.g,
            b: p.b,
            alpha: p.alpha,
        }).collect();

        let prep_secondary: Vec<PreparedPixel> = tmpl.secondary_anchors.iter().map(|p| PreparedPixel {
            byte_offset: (p.offset_y as usize) * screen_stride + (p.offset_x as usize) * 4,
            r: p.r,
            g: p.g,
            b: p.b,
            alpha: p.alpha,
        }).collect();

        let prep_samples: Vec<PreparedPixel> = tmpl.samples.iter().map(|p| PreparedPixel {
            byte_offset: (p.offset_y as usize) * screen_stride + (p.offset_x as usize) * 4,
            r: p.r,
            g: p.g,
            b: p.b,
            alpha: p.alpha,
        }).collect();

        let prep_active: Vec<PreparedPixel> = tmpl.active_pixels.iter().map(|p| PreparedPixel {
            byte_offset: (p.offset_y as usize) * screen_stride + (p.offset_x as usize) * 4,
            r: p.r,
            g: p.g,
            b: p.b,
            alpha: p.alpha,
        }).collect();

        let is_all_opaque = tmpl.is_all_opaque;
        let primary_max_diff = tmpl.primary_max_diff;
        let secondary_max_diff = tmpl.secondary_max_diff;
        let sample_max_diff = tmpl.sample_max_diff;
        let max_total_diff = tmpl.max_total_diff;

        let global_best_score = AtomicU32::new((threshold * 10000.0) as u32);

        let (best_score, best_x, best_y) = (roi_y1..=max_y)
            .into_par_iter()
            .map(|y| {
                let mut local_best_score: f32 = 0.0;
                let mut local_best_x: u32 = roi_x1;
                let mut local_best_y: u32 = y;
                let y_base_idx = (y as usize) * screen_stride;

                for x in roi_x1..=max_x {
                    let g_best_u32 = global_best_score.load(Ordering::Relaxed);
                    let min_score = (g_best_u32 as f32 / 10000.0).max(threshold);
                    let base_idx = y_base_idx + (x as usize) * 4;

                    // Stage 1: Primary Anchors Check (8 points)
                    let max_allowed_p = (primary_max_diff as f32 * (1.0 - min_score)) as u32;
                    let mut p_diff: u32 = 0;
                    let mut fail = false;

                    for a in &prep_primary {
                        let s_idx = base_idx + a.byte_offset;
                        let s_r = screen_raw[s_idx];
                        let s_g = screen_raw[s_idx + 1];
                        let s_b = screen_raw[s_idx + 2];
                        let pd = s_r.abs_diff(a.r) as u32 + s_g.abs_diff(a.g) as u32 + s_b.abs_diff(a.b) as u32;

                        if a.alpha == 255 {
                            p_diff += pd;
                        } else {
                            p_diff += (pd * a.alpha as u32) / 255;
                        }

                        if p_diff > max_allowed_p {
                            fail = true;
                            break;
                        }
                    }
                    if fail {
                        continue;
                    }

                    // Stage 2: Secondary Anchors Check (16 points)
                    if prep_secondary.len() > prep_primary.len() {
                        let max_allowed_s = (secondary_max_diff as f32 * (1.0 - min_score)) as u32;
                        let mut s_diff: u32 = 0;

                        for a in &prep_secondary {
                            let s_idx = base_idx + a.byte_offset;
                            let s_r = screen_raw[s_idx];
                            let s_g = screen_raw[s_idx + 1];
                            let s_b = screen_raw[s_idx + 2];
                            let pd = s_r.abs_diff(a.r) as u32 + s_g.abs_diff(a.g) as u32 + s_b.abs_diff(a.b) as u32;

                            if a.alpha == 255 {
                                s_diff += pd;
                            } else {
                                s_diff += (pd * a.alpha as u32) / 255;
                            }

                            if s_diff > max_allowed_s {
                                fail = true;
                                break;
                            }
                        }
                        if fail {
                            continue;
                        }
                    }

                    // Stage 3: Subsampled Grid Check (up to 49 points)
                    if prep_samples.len() > prep_secondary.len() {
                        let max_allowed_sample = (sample_max_diff as f32 * (1.0 - min_score)) as u32;
                        let mut sample_diff: u32 = 0;

                        for s in &prep_samples {
                            let s_idx = base_idx + s.byte_offset;
                            let s_r = screen_raw[s_idx];
                            let s_g = screen_raw[s_idx + 1];
                            let s_b = screen_raw[s_idx + 2];
                            let pd = s_r.abs_diff(s.r) as u32 + s_g.abs_diff(s.g) as u32 + s_b.abs_diff(s.b) as u32;

                            if s.alpha == 255 {
                                sample_diff += pd;
                            } else {
                                sample_diff += (pd * s.alpha as u32) / 255;
                            }

                            if sample_diff > max_allowed_sample {
                                fail = true;
                                break;
                            }
                        }
                        if fail {
                            continue;
                        }
                    }

                    // Stage 4: Full Evaluation with Early Exit
                    let max_allowed_diff = (max_total_diff as f32 * (1.0 - min_score)) as u32;
                    let mut total_diff: u32 = 0;
                    let mut early_exit = false;

                    if is_all_opaque {
                        for p in &prep_active {
                            let s_idx = base_idx + p.byte_offset;
                            let s_r = screen_raw[s_idx];
                            let s_g = screen_raw[s_idx + 1];
                            let s_b = screen_raw[s_idx + 2];
                            total_diff += s_r.abs_diff(p.r) as u32 + s_g.abs_diff(p.g) as u32 + s_b.abs_diff(p.b) as u32;

                            if total_diff > max_allowed_diff {
                                early_exit = true;
                                break;
                            }
                        }
                    } else {
                        for p in &prep_active {
                            let s_idx = base_idx + p.byte_offset;
                            let s_r = screen_raw[s_idx];
                            let s_g = screen_raw[s_idx + 1];
                            let s_b = screen_raw[s_idx + 2];
                            let pd = s_r.abs_diff(p.r) as u32 + s_g.abs_diff(p.g) as u32 + s_b.abs_diff(p.b) as u32;
                            total_diff += (pd * p.alpha as u32) / 255;

                            if total_diff > max_allowed_diff {
                                early_exit = true;
                                break;
                            }
                        }
                    }

                    if early_exit {
                        continue;
                    }

                    let score = (1.0 - (total_diff as f32 / max_total_diff as f32)).clamp(0.0, 1.0);
                    if score > local_best_score {
                        local_best_score = score;
                        local_best_x = x;
                        local_best_y = y;
                        global_best_score.fetch_max((score * 10000.0) as u32, Ordering::Relaxed);
                    }
                }

                (local_best_score, local_best_x, local_best_y)
            })
            .reduce(
                || (0.0f32, roi_x1, roi_y1),
                |a, b| if b.0 > a.0 { b } else { a },
            );

        let matched = best_score >= threshold;
        MatchResult {
            matched,
            confidence: best_score,
            box_x: best_x,
            box_y: best_y,
            width: tw,
            height: th,
            center_x: best_x + tw / 2,
            center_y: best_y + th / 2,
            template_path: template_path.to_string(),
        }
    }
}
