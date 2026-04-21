//! Efficient drawing of larger pixel areas.

use std::fmt;

use egui::{
    Color32, ColorImage, Image, Rect, TextureHandle, TextureOptions, Ui, load::SizedTexture, vec2,
};

use crate::cache::Cached;

/// Represents an image that is cached if possible and otherwise drawn again.
pub struct CachedImage<T: PartialEq> {
    /// The cached image.
    cached_image: Cached<(usize, usize, T), TextureHandle>,
}

impl<T: PartialEq> CachedImage<T> {
    /// Creates a new cached image.
    pub fn new() -> CachedImage<T> {
        CachedImage {
            cached_image: Cached::new(),
        }
    }

    /// Renders the image or returns the cached image.
    ///
    /// Will re-render the image if `width`, `height` or `params` have changed since the last call
    /// to render.
    /// The supplied closure is called with the `x` and `y` coordinates of the pixel for which the
    /// color is then returned.
    fn rendered(
        &mut self,
        ui: &mut Ui,
        width: usize,
        height: usize,
        params: T,
        mut render: impl FnMut(usize, usize) -> Color32,
    ) -> Image<'static> {
        let handle = self.cached_image.get((width, height, params), |old| {
            let mut bytes = vec![0; width * height * 4];

            for y in 0..height {
                for x in 0..width {
                    let start = (width * y + x) * 4;
                    let color = render(x, y);

                    bytes[start] = color.r();
                    bytes[start + 1] = color.g();
                    bytes[start + 2] = color.b();
                    bytes[start + 3] = color.a();
                }
            }

            let img = ColorImage::from_rgba_premultiplied([width, height], &bytes);

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
    pub fn paint_at(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        params: T,
        render: impl FnMut(usize, usize) -> Color32,
    ) {
        let width = rect.width().trunc() as usize;
        let height = rect.height().trunc() as usize;

        self.rendered(ui, width, height, params, render)
            .paint_at(ui, rect);
    }

    /// Signals that the image requires a repaint.
    pub fn require_repaint(&mut self) {
        self.cached_image.invalidate();
    }
}

impl<T: PartialEq> Default for CachedImage<T> {
    fn default() -> Self {
        CachedImage::new()
    }
}

impl<T: PartialEq> fmt::Debug for CachedImage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedImage").finish_non_exhaustive()
    }
}
