//! Efficient drawing of larger pixel areas.

use std::{cell::RefCell, fmt};

use egui::{
    Color32, ColorImage, Image, Rect, TextureHandle, TextureOptions, Ui, load::SizedTexture, vec2,
};

use crate::{
    cache::Cached,
    gui::color::{self, LerpStrength},
    state::Settings,
};

/// Represents an image that is cached if possible and otherwise drawn again.
pub struct CachedImage<T: PartialEq> {
    /// The cached image.
    cached_image: Cached<(usize, usize, T), TextureHandle>,
    /// Whether to keep the raw image data for later use.
    keep_raw: bool,
    /// The last computed raw image, if `keep_raw` is `true`.
    raw_image: Option<ColorImage>,
}

impl<T: PartialEq> CachedImage<T> {
    /// Creates a new cached image.
    pub fn new(keep_raw: bool) -> CachedImage<T> {
        CachedImage {
            cached_image: Cached::new(),
            keep_raw,
            raw_image: None,
        }
    }

    /// Renders the image or returns the cached image.
    ///
    /// Will re-render the image if `width`, `height` or `params` have changed since the last call
    /// to render.
    /// The supplied closure is called with the `x` and `y` coordinates of the pixel for which the
    /// color is then returned.
    #[inline]
    fn rendered<Ctx>(
        &mut self,
        ui: &mut Ui,
        width: usize,
        height: usize,
        params: T,
        ctx: impl FnOnce() -> Ctx,
        mut render: impl FnMut(&Ctx, usize, usize) -> Color32,
    ) -> Image<'static> {
        let handle = self.cached_image.get((width, height, params), |old| {
            let mut bytes = vec![0; width * height * 4];

            let ctx = ctx();

            for y in 0..height {
                for x in 0..width {
                    let start = (width * y + x) * 4;
                    let color = render(&ctx, x, y);

                    bytes[start] = color.r();
                    bytes[start + 1] = color.g();
                    bytes[start + 2] = color.b();
                    bytes[start + 3] = color.a();
                }
            }

            let img = ColorImage::from_rgba_premultiplied([width, height], &bytes);

            if self.keep_raw {
                self.raw_image = Some(img.clone());
            }

            match old {
                Some(handle) => {
                    let mut handle = handle.clone();
                    handle.set(img, TextureOptions::NEAREST);

                    handle
                }
                None => ui
                    .ctx()
                    .load_texture("cached_image", img, TextureOptions::NEAREST),
            }
        });

        Image::new(SizedTexture {
            id: handle.id(),
            size: vec2(width as f32, height as f32),
        })
    }

    /// Paints the image to the `Ui` at the given `Rect`.
    ///
    /// Will re-render the image if the dimensions or `params` have changed since the last call
    /// to render.
    /// The supplied closure is called with the `x` and `y` coordinates of the pixel for which the
    /// color is then returned.
    #[inline]
    pub fn paint_at<Ctx>(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        params: T,
        ctx: impl FnOnce() -> Ctx,
        render: impl FnMut(&Ctx, usize, usize) -> Color32,
    ) {
        let width = rect.width().trunc() as usize;
        let height = rect.height().trunc() as usize;

        self.rendered(ui, width, height, params, ctx, render)
            .paint_at(ui, rect);
    }

    /// Returns the raw image data of the last computed version.
    ///
    /// # Panics
    /// This function panics if `keep_raw` is `false` or if no image was computed yet.
    pub fn raw(&self) -> &ColorImage {
        self.raw_image
            .as_ref()
            .expect("`keep_raw` is `false` or the image is not computed yet")
    }

    /// Signals that the image requires a repaint.
    pub fn require_repaint(&mut self) {
        self.cached_image.invalidate();
    }
}

impl<T: PartialEq> fmt::Debug for CachedImage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedImage").finish_non_exhaustive()
    }
}

/// Gaussian kernel for σ=0.8, radius 5. Sums to 1.0.
const KERNEL: [f32; 5] = [
    0.021_929_65,
    0.228_512_14,
    0.499_116_42,
    0.228_512_14,
    0.021_929_65,
];

/// The radius of the Gaussian kernel.
const KERNEL_RADIUS: usize = (KERNEL.len() - 1) / 2;

thread_local! {
    /// A scratch buffer used by the blurring function.
    static BLUR_SCRATCH: RefCell<Vec<[f32; 3]>> = const { RefCell::new(Vec::new()) };
}

/// Applies a separable Gaussian blur to `image` and tints the result.
///
/// Uses a two-pass approach (horizontal then vertical) with a shared
/// `KERNEL`. A thread-local ring buffer of `KERNEL.len()` horizontally-blurred
/// rows avoids allocating a full intermediate image.
///
/// Interior pixels use the kernel directly (which must sum to 1.0).
/// Border pixels (within `KERNEL_RADIUS` of any edge) renormalize
/// the truncated kernel weights. Images smaller than the kernel
/// diameter are handled entirely via the border path.
///
/// Every output pixel is tinted toward `scrollbar_non_selected_color`
/// by `scrollbar_non_selected_tint_strength` from `settings`.
pub fn blur_image(image: &ColorImage, settings: &Settings) -> ColorImage {
    let [w, h] = image.size;

    let tint_color = settings.scrollbar_non_selected_color();
    let tint_strength = settings.scrollbar_non_selected_tint_strength();

    BLUR_SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        let ring_size = KERNEL.len() * w;
        if scratch.len() < ring_size {
            scratch.resize(ring_size, [0.0; 3]);
        }

        let pixels = &image.pixels;

        // Pre-fill first KERNEL.len() rows (or h if smaller)
        let prefill = KERNEL.len().min(h);
        for row in 0..prefill {
            blur_row_horiz(pixels, &mut scratch, row, w);
        }

        let mut out = ColorImage::filled(image.size, Color32::TRANSPARENT);

        for y in 0..h {
            // --- Vertical blur for row y ---
            if h > 2 * KERNEL_RADIUS && y >= KERNEL_RADIUS && y < h - KERNEL_RADIUS {
                // Interior: all kernel rows present, full kernel sums to 1.0.
                // Manually unrolled — a loop via std::array::from_fn regresses ~1ms/3MP.
                // This assert ensures that a change in KERNEL size needs to be addressed here
                assert_eq!(KERNEL.len(), 5);

                let row0 = &scratch[ring_offset(y - 2, w)..][..w];
                let row1 = &scratch[ring_offset(y - 1, w)..][..w];
                let row2 = &scratch[ring_offset(y, w)..][..w];
                let row3 = &scratch[ring_offset(y + 1, w)..][..w];
                let row4 = &scratch[ring_offset(y + 2, w)..][..w];

                let out_row = &mut out.pixels[y * w..(y + 1) * w];

                for x in 0..w {
                    let r = row0[x][0] * KERNEL[0]
                        + row1[x][0] * KERNEL[1]
                        + row2[x][0] * KERNEL[2]
                        + row3[x][0] * KERNEL[3]
                        + row4[x][0] * KERNEL[4];
                    let g = row0[x][1] * KERNEL[0]
                        + row1[x][1] * KERNEL[1]
                        + row2[x][1] * KERNEL[2]
                        + row3[x][1] * KERNEL[3]
                        + row4[x][1] * KERNEL[4];
                    let b = row0[x][2] * KERNEL[0]
                        + row1[x][2] * KERNEL[1]
                        + row2[x][2] * KERNEL[2]
                        + row3[x][2] * KERNEL[3]
                        + row4[x][2] * KERNEL[4];

                    out_row[x] = finish_pixel(r, g, b, tint_color, tint_strength);
                }
            } else {
                // Border rows: clamped access with weight normalization
                let y_min = y.saturating_sub(KERNEL_RADIUS);
                let y_max = (y + KERNEL_RADIUS + 1).min(h);
                let out_row = &mut out.pixels[y * w..(y + 1) * w];

                for x in 0..w {
                    let mut rgb = [0.0, 0.0, 0.0];
                    let mut weight_sum = 0.0;

                    for yi in y_min..y_max {
                        let kernel_weight = KERNEL[yi + KERNEL_RADIUS - y];
                        let pixel = scratch[ring_offset(yi, w) + x];

                        for (out, channel) in rgb.iter_mut().zip(pixel) {
                            *out += channel * kernel_weight;
                        }

                        weight_sum += kernel_weight;
                    }

                    let inv = 1.0 / weight_sum;
                    for out in rgb.iter_mut() {
                        *out *= inv;
                    }

                    let [r, g, b] = rgb;
                    out_row[x] = finish_pixel(r, g, b, tint_color, tint_strength);
                }
            }

            // Advance: bring in the next row entering the window
            let next = y + KERNEL_RADIUS + 1;
            if next >= prefill && next < h {
                blur_row_horiz(pixels, &mut scratch, next, w);
            }
        }

        out
    })
}

/// Computes the offset for the given `row` into the ring buffer, where rows have width `w`.
#[inline]
fn ring_offset(row: usize, w: usize) -> usize {
    (row % KERNEL.len()) * w
}

/// Takes the pixel values as `f32`, applies tinting and converts back to a color.
#[inline]
fn finish_pixel(r: f32, g: f32, b: f32, tint: Color32, tint_strength: LerpStrength) -> Color32 {
    let blurred = Color32::from_rgb((r + 0.5) as u8, (g + 0.5) as u8, (b + 0.5) as u8);

    color::lerp(blurred, tint, tint_strength)
}

/// Applies the horizontal blur kernel to row `src_y` of `pixels`,
/// writing the result into the ring buffer slot for that row.
///
/// Interior pixels (those at least `KERNEL_RADIUS` from either edge)
/// use the full kernel directly. Border pixels use [`horiz_border`],
/// which renormalizes the truncated kernel weights.
///
/// If the image is narrower than the kernel diameter (`w <= 2 * KERNEL_RADIUS`),
/// all pixels are treated as border pixels.#[inline]
fn blur_row_horiz(pixels: &[Color32], scratch: &mut [[f32; 3]], src_y: usize, w: usize) {
    let slot = ring_offset(src_y, w);
    let row = src_y * w;

    if w > 2 * KERNEL_RADIUS {
        for x in 0..KERNEL_RADIUS {
            scratch[slot + x] = horiz_border(pixels, row, x, w);
        }

        let src = &pixels[row..row + w];
        for x in KERNEL_RADIUS..(w - KERNEL_RADIUS) {
            let mut r = 0.0;
            let mut g = 0.0;
            let mut b = 0.0;

            for k in 0..KERNEL.len() {
                let color = src[x - KERNEL_RADIUS + k];
                let kernel_weight = KERNEL[k];

                r += color.r() as f32 * kernel_weight;
                g += color.g() as f32 * kernel_weight;
                b += color.b() as f32 * kernel_weight;
            }

            scratch[slot + x] = [r, g, b];
        }

        for x in (w - KERNEL_RADIUS)..w {
            scratch[slot + x] = horiz_border(pixels, row, x, w);
        }
    } else {
        for x in 0..w {
            scratch[slot + x] = horiz_border(pixels, row, x, w);
        }
    }
}

/// Horizontal blur for a single pixel near the left or right edge of a row.
///
/// Because the kernel extends beyond the image boundary, only the
/// in-bounds portion is summed and the result is renormalized by the
/// actual weight sum (rather than assuming the full kernel sums to 1.0).
#[inline]
fn horiz_border(pixels: &[Color32], row: usize, x: usize, w: usize) -> [f32; 3] {
    let x_min = x.saturating_sub(KERNEL_RADIUS);
    let x_max = (x + KERNEL_RADIUS + 1).min(w);

    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;

    let mut weight_sum = 0.0;
    for xi in x_min..x_max {
        let kernel_weight = KERNEL[xi + KERNEL_RADIUS - x];
        let color = pixels[row + xi];

        r += color.r() as f32 * kernel_weight;
        g += color.g() as f32 * kernel_weight;
        b += color.b() as f32 * kernel_weight;

        weight_sum += kernel_weight;
    }

    let inv = 1.0 / weight_sum;
    [r * inv, g * inv, b * inv]
}
