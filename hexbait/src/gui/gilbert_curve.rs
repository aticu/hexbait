//! Gilbert curve — a space-filling curve for arbitrary-sized rectangles.
//!
//! Based on William Gilbert's 1984 generalization of the Hilbert curve.
//! Unlike the classic Hilbert curve (which requires power-of-2 dimensions),
//! the Gilbert curve produces a Hamiltonian path through any W×H grid.
//!
//! The algorithm works by recursively bisecting the rectangle along its
//! longer axis, flipping traversal directions so that the end of one
//! sub-rectangle meets the start of the next, forming a continuous path.

/// Contains a gilbert curve.
pub struct GilbertCurve {
    /// The map from an index in the 1D domain to a point in the 2D domain.
    index_to_point: Vec<Point>,
    /// The map from a point in the 2D domain to an index in the 1D domain.
    point_to_index: Vec<usize>,
    /// The width of the gilbert map.
    width: u32,
}

impl GilbertCurve {
    /// Generate a Gilbert curve that visits every cell in a `width × height` grid.
    ///
    /// # Panics
    /// Panics if `width` or `height` is zero or too large to fit into `i32`.
    pub fn compute(width: u32, height: u32) -> GilbertCurve {
        let index_to_point = gilbert2d(
            i32::try_from(width).expect("width should fit into i32"),
            i32::try_from(height).expect("height should fit into i32"),
        );
        let mut point_to_index = vec![0; (height * width) as usize];

        for (i, point) in index_to_point.iter().enumerate() {
            point_to_index[point.x as usize + (point.y as u32 * width) as usize] = i;
        }

        GilbertCurve {
            index_to_point,
            point_to_index,
            width,
        }
    }

    /// Returns the index corresponding to the given point.
    ///
    /// # Panics
    /// May panic if `x >= width` or `y >= height`.
    #[track_caller]
    pub fn index_from_point(&self, x: usize, y: usize) -> usize {
        self.point_to_index[x + y * self.width as usize]
    }

    /// Returns the point corresponding to the given index.
    ///
    /// # Panics
    /// May panic if the index is larger than the number of points.
    #[track_caller]
    pub fn point_from_index(&self, index: usize) -> Point {
        self.index_to_point[index]
    }
}

/// A 2D point on the grid (also used to represent directional vectors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// The x coordinate of the point.
    pub x: i32,
    /// The y coordinate of the point.
    pub y: i32,
}

/// A rectangular region to fill, defined by an origin and two extent vectors.
///
/// The curve fills the parallelogram spanned by `primary` and `secondary`
/// starting from `origin`. In practice these are always axis-aligned
/// rectangles, but the math works on arbitrary parallelograms.
#[derive(Debug, Clone, Copy)]
struct Region {
    /// Top-left corner (starting point of the curve in this region).
    origin: Point,
    /// Extent vector along the primary (longer) axis.
    /// e.g. (width, 0) means "width cells to the right".
    primary: Point,
    /// Extent vector along the secondary (shorter) axis.
    /// e.g. (0, height) means "height cells downward".
    secondary: Point,
}

impl Region {
    /// Length of the primary axis.
    fn primary_len(&self) -> i32 {
        (self.primary.x + self.primary.y).abs()
    }

    /// Length of the secondary axis.
    fn secondary_len(&self) -> i32 {
        (self.secondary.x + self.secondary.y).abs()
    }

    /// Unit step direction along the primary axis (each component is -1, 0, or 1).
    fn primary_step(&self) -> Point {
        Point {
            x: self.primary.x.signum(),
            y: self.primary.y.signum(),
        }
    }

    /// Unit step direction along the secondary axis.
    fn secondary_step(&self) -> Point {
        Point {
            x: self.secondary.x.signum(),
            y: self.secondary.y.signum(),
        }
    }
}

/// Generate a Gilbert curve that visits every cell in a `width × height` grid.
///
/// Returns a `Vec<Point>` of length `width * height`, starting at (0, 0).
///
/// # Panics
/// Panics if `width` or `height` is zero.
fn gilbert2d(width: i32, height: i32) -> Vec<Point> {
    assert!(width > 0 && height > 0, "dimensions must be positive");

    let mut points = Vec::with_capacity((width * height) as usize);

    let region = if width >= height {
        Region {
            origin: Point { x: 0, y: 0 },
            primary: Point { x: width, y: 0 },
            secondary: Point { x: 0, y: height },
        }
    } else {
        Region {
            origin: Point { x: 0, y: 0 },
            primary: Point { x: 0, y: height },
            secondary: Point { x: width, y: 0 },
        }
    };

    generate(region, &mut points);

    points
}

/// Core recursive generator.
///
/// Fills `region` by either walking linearly (base case) or splitting
/// it into sub-regions with swapped/reversed axes so the path turns
/// corners and remains contiguous.
fn generate(region: Region, out: &mut Vec<Point>) {
    let w = region.primary_len();
    let h = region.secondary_len();
    let step_p = region.primary_step();
    let step_s = region.secondary_step();

    // --- Base cases: single row or column — just walk straight. --------

    if h == 1 {
        let (mut cx, mut cy) = (region.origin.x, region.origin.y);
        for _ in 0..w {
            out.push(Point { x: cx, y: cy });
            cx += step_p.x;
            cy += step_p.y;
        }
        return;
    }

    if w == 1 {
        let (mut cx, mut cy) = (region.origin.x, region.origin.y);
        for _ in 0..h {
            out.push(Point { x: cx, y: cy });
            cx += step_s.x;
            cy += step_s.y;
        }
        return;
    }

    // --- Recursive case: split the region. -----------------------------

    // Half-extents along each axis.
    let mut half_primary = Point {
        x: region.primary.x / 2,
        y: region.primary.y / 2,
    };
    let mut half_secondary = Point {
        x: region.secondary.x / 2,
        y: region.secondary.y / 2,
    };

    let half_w = (half_primary.x + half_primary.y).abs();
    let half_h = (half_secondary.x + half_secondary.y).abs();

    if 2 * w > 3 * h {
        // Wide rectangle — split only along the primary axis into two halves.
        // No axis-swapping needed; the two sub-curves share a full edge.

        if (half_w & 1) != 0 && w > 2 {
            // Nudge to an even split so endpoints align.
            half_primary.x += step_p.x;
            half_primary.y += step_p.y;
        }

        let remainder_primary = Point {
            x: region.primary.x - half_primary.x,
            y: region.primary.y - half_primary.y,
        };

        // Left half.
        generate(
            Region {
                origin: region.origin,
                primary: half_primary,
                secondary: region.secondary,
            },
            out,
        );

        // Right half.
        generate(
            Region {
                origin: Point {
                    x: region.origin.x + half_primary.x,
                    y: region.origin.y + half_primary.y,
                },
                primary: remainder_primary,
                secondary: region.secondary,
            },
            out,
        );
    } else {
        // Roughly square — split along both axes into three sub-regions.
        // Axes are swapped in the corner blocks to force the curve to turn.

        if (half_h & 1) != 0 && h > 2 {
            half_secondary.x += step_s.x;
            half_secondary.y += step_s.y;
        }

        let remainder_primary = Point {
            x: region.primary.x - half_primary.x,
            y: region.primary.y - half_primary.y,
        };
        let remainder_secondary = Point {
            x: region.secondary.x - half_secondary.x,
            y: region.secondary.y - half_secondary.y,
        };

        // 1) First corner — axes swapped so the curve enters turning.
        generate(
            Region {
                origin: region.origin,
                primary: half_secondary,
                secondary: half_primary,
            },
            out,
        );

        // 2) Middle strip — original axis orientation.
        generate(
            Region {
                origin: Point {
                    x: region.origin.x + half_secondary.x,
                    y: region.origin.y + half_secondary.y,
                },
                primary: region.primary,
                secondary: remainder_secondary,
            },
            out,
        );

        // 3) Far corner — axes swapped AND reversed so the curve
        //    enters from the correct edge and closes the path.
        generate(
            Region {
                origin: Point {
                    x: region.origin.x
                        + (region.primary.x - step_p.x)
                        + (half_secondary.x - step_s.x),
                    y: region.origin.y
                        + (region.primary.y - step_p.y)
                        + (half_secondary.y - step_s.y),
                },
                primary: Point {
                    x: -half_secondary.x,
                    y: -half_secondary.y,
                },
                secondary: Point {
                    x: -remainder_primary.x,
                    y: -remainder_primary.y,
                },
            },
            out,
        );
    }
}
