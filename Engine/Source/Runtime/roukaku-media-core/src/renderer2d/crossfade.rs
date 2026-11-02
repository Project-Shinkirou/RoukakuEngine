use super::*;

/// Both crossfade inputs plus the shared interpolation parameters.
pub(super) struct Sources<'a> {
    pub(super) previous: &'a TextureFrame,
    pub(super) current: &'a TextureFrame,
    pub(super) source: Option<RectI>,
    pub(super) progress: f32,
    pub(super) opacity: f32,
}

impl Sources<'_> {
    fn sample(&self, u: f32, v: f32) -> ([u8; 4], [u8; 4]) {
        let sample = |frame: &TextureFrame| {
            let rect = self
                .source
                .unwrap_or(RectI::new(0, 0, frame.width, frame.height));
            let x = rect.x as usize
                + ((u * rect.width as f32).floor() as usize).min(rect.width as usize - 1);
            let y = rect.y as usize
                + ((v * rect.height as f32).floor() as usize).min(rect.height as usize - 1);
            let offset = (y * frame.width as usize + x) * 4;
            <[u8; 4]>::try_from(&frame.rgba8[offset..offset + 4])
                .expect("validated texture coordinates")
        };
        (sample(self.previous), sample(self.current))
    }

    /// Lerps the two straight-alpha samples in premultiplied space, then composes
    /// the result source-over onto the straight-alpha target pixel.
    fn blend(&self, pixel: &mut [u8], u: f32, v: f32) {
        let (previous, current) = self.sample(u, v);
        let previous_weight = f32::from(previous[3]) / 255.0 * (1.0 - self.progress) * self.opacity;
        let current_weight = f32::from(current[3]) / 255.0 * self.progress * self.opacity;
        let alpha = previous_weight + current_weight;
        let background_weight = f32::from(pixel[3]) / 255.0 * (1.0 - alpha);
        let output_alpha = alpha + background_weight;
        for channel in 0..3 {
            let premultiplied = f32::from(previous[channel]) * previous_weight
                + f32::from(current[channel]) * current_weight
                + f32::from(pixel[channel]) * background_weight;
            pixel[channel] = if output_alpha == 0.0 {
                0
            } else {
                (premultiplied / output_alpha).round().clamp(0.0, 255.0) as u8
            };
        }
        pixel[3] = (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

fn pixel(target: &mut [u8], width: usize, x: usize, y: usize) -> &mut [u8] {
    let offset = (y * width + x) * 4;
    &mut target[offset..offset + 4]
}

pub(super) fn draw_sprite(
    target: &mut [u8],
    target_width: usize,
    target_height: usize,
    clip: RectI,
    transform: Transform2D,
    destination: RectI,
    sources: Sources<'_>,
) -> Result<(), MediaError> {
    let inverse = transform
        .inverse()
        .ok_or_else(|| MediaError::message("ROUKAKU_MEDIA_TRANSFORM: transform is singular"))?;
    let bounds = intersection(clip, transformed_bounds(transform, destination)?);
    let top = bounds.y.max(0) as usize;
    let bottom = (bounds.y + bounds.height as i32).clamp(0, target_height as i32) as usize;
    let left = bounds.x.max(0) as usize;
    let right = (bounds.x + bounds.width as i32).clamp(0, target_width as i32) as usize;
    for y in top..bottom {
        for x in left..right {
            let (local_x, local_y) = inverse.point(x as f32 + 0.5, y as f32 + 0.5);
            let u = (local_x - destination.x as f32) / destination.width as f32;
            let v = (local_y - destination.y as f32) / destination.height as f32;
            if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
                sources.blend(pixel(target, target_width, x, y), u, v);
            }
        }
    }
    Ok(())
}

/// Corner order is top-left, top-right, bottom-left, bottom-right; the quad is
/// split along the top-right/bottom-left diagonal with a top-left fill rule so
/// shared edges are drawn exactly once.
pub(super) fn draw_quad(
    target: &mut [u8],
    width: usize,
    height: usize,
    clip: RectI,
    transform: Transform2D,
    positions: [[f32; 2]; 4],
    sources: Sources<'_>,
) -> Result<(), MediaError> {
    const UV: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    let positions = positions.map(|[x, y]| {
        let (x, y) = transform.point(x, y);
        [x, y]
    });
    if positions.iter().flatten().any(|value| !value.is_finite()) {
        return Err(MediaError::message(
            "ROUKAKU_MEDIA_CROSSFADE_VERTEX: transformed crossfade quad is not finite",
        ));
    }
    let extent = |axis: usize, pick: fn(f32, f32) -> f32, start: f32| {
        positions.iter().map(|point| point[axis]).fold(start, pick)
    };
    let left = (extent(0, f32::min, f32::INFINITY).floor() as i32)
        .max(clip.x)
        .max(0);
    let top = (extent(1, f32::min, f32::INFINITY).floor() as i32)
        .max(clip.y)
        .max(0);
    let right = (extent(0, f32::max, f32::NEG_INFINITY).ceil() as i32)
        .min(clip.x.saturating_add_unsigned(clip.width))
        .min(width as i32);
    let bottom = (extent(1, f32::max, f32::NEG_INFINITY).ceil() as i32)
        .min(clip.y.saturating_add_unsigned(clip.height))
        .min(height as i32);
    for y in top..bottom {
        for x in left..right {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            for [a, mut b, mut c] in [[0, 1, 2], [2, 1, 3]] {
                let mut area = edge(positions[a], positions[b], positions[c]);
                if area.abs() < f32::EPSILON {
                    continue;
                }
                if area > 0.0 {
                    std::mem::swap(&mut b, &mut c);
                    area = -area;
                }
                let edges = [(b, c), (c, a), (a, b)];
                let values =
                    edges.map(|(start, end)| edge(positions[start], positions[end], point));
                let outside = values.iter().zip(edges).any(|(&value, (start, end))| {
                    let [x0, y0] = positions[start];
                    let [x1, y1] = positions[end];
                    value > 0.0 || value == 0.0 && !(y1 < y0 || y1 == y0 && x1 > x0)
                });
                if outside {
                    continue;
                }
                let [wa, wb, wc] = values.map(|value| value / area);
                let u = (wa * UV[a][0] + wb * UV[b][0] + wc * UV[c][0]).clamp(0.0, 1.0);
                let v = (wa * UV[a][1] + wb * UV[b][1] + wc * UV[c][1]).clamp(0.0, 1.0);
                sources.blend(pixel(target, width, x as usize, y as usize), u, v);
            }
        }
    }
    Ok(())
}

fn edge(start: [f32; 2], end: [f32; 2], point: [f32; 2]) -> f32 {
    (end[0] - start[0]) * (point[1] - start[1]) - (end[1] - start[1]) * (point[0] - start[0])
}
