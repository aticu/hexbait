//! Efficient drawing of larger pixel areas.

use std::fmt;

use egui::{
    Color32, ColorImage, Image, Rect, TextureHandle, TextureOptions, Ui, load::SizedTexture, vec2,
};

/// Represents an image that is cached if possible and otherwise drawn again.
pub struct CachedImage<T: PartialEq> {
    /// The texture handle for the image.
    texture_handle: Option<TextureHandle>,
    /// The last width of the rendered image.
    width: usize,
    /// The last height of the rendered image.
    height: usize,
    /// The parameters that determine the staleness of the image.
    params: Option<T>,
    /// Can be set for images where the parameters are only known during rendering.
    require_repaint: bool,
}

impl<T: PartialEq> CachedImage<T> {
    /// Creates a new cached image.
    pub fn new() -> CachedImage<T> {
        CachedImage {
            texture_handle: None,
            width: 0,
            height: 0,
            params: None,
            require_repaint: false,
        }
    }

    /// Renders the image or returns the cached image.
    ///
    /// Will re-render the image if `width`, `height` or `params` have changed since the last call
    /// to render.
    /// The supplied closure is called with the `x` and `y` coordinates of the pixel for which the
    /// color is then returned.
    pub fn rendered(
        &mut self,
        ui: &mut Ui,
        width: usize,
        height: usize,
        params: T,
        mut render: impl FnMut(usize, usize) -> Color32,
    ) -> Image<'static> {
        let can_keep = self.texture_handle.is_some()
            && !self.require_repaint
            && width == self.width
            && height == self.height
            && Some(&params) == self.params.as_ref();

        if can_keep {
            let id = self.texture_handle.as_ref().unwrap().id();
            return Image::new(SizedTexture {
                id,
                size: vec2(width as f32, height as f32),
            });
        }

        self.width = width;
        self.height = height;
        self.params = Some(params);

        let mut bytes = vec![0; width * height * 4];

        for x in 0..width {
            for y in 0..height {
                let start = (width * y + x) * 4;
                let color = render(x, y);

                bytes[start] = color.r();
                bytes[start + 1] = color.g();
                bytes[start + 2] = color.b();
                bytes[start + 3] = color.a();
            }
        }

        let img = ColorImage::from_rgba_premultiplied([width, height], &bytes);

        let id = match &mut self.texture_handle {
            Some(handle) => {
                handle.set(img, TextureOptions::NEAREST);
                handle.id()
            }
            None => {
                self.texture_handle = Some(ui.ctx().load_texture(
                    "cached_image",
                    img,
                    TextureOptions::NEAREST,
                ));

                self.texture_handle.as_ref().unwrap().id()
            }
        };

        Image::new(SizedTexture {
            id,
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

    /// Updates whether the image requires a repaint.
    pub fn require_repaint(&mut self, require_repaint: bool) {
        self.require_repaint = require_repaint;
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
