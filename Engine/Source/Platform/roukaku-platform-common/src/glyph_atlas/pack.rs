use super::*;
use roukaku_platform::PlatformError;
use std::collections::BTreeMap;

/// Square extents tried when the first atlas is built.
pub(crate) const INITIAL_ATLAS_EXTENTS: [(u32, u32); 3] = [
    (MIN_ATLAS_SIDE, MIN_ATLAS_SIDE),
    (2048, 2048),
    (ATLAS_SIDE, ATLAS_SIDE),
];

/// Extents tried when an existing atlas runs out of room; a repack never
/// shrinks below the current area.
pub(crate) const REPACK_ATLAS_EXTENTS: [(u32, u32); 6] = [
    (MIN_ATLAS_SIDE, MIN_ATLAS_SIDE),
    (2048, 2048),
    (ATLAS_SIDE, 2048),
    (ATLAS_SIDE, ATLAS_SIDE),
    (MAX_ATLAS_WIDTH, 2048),
    (MAX_ATLAS_WIDTH, ATLAS_SIDE),
];

/// Packs every resource onto the first single-page extent that holds them
/// all. Only a set that overflows every single-page extent spills onto
/// additional pages of at most `max_dimension` x `ATLAS_SIDE`.
pub(crate) fn pack_atlas(
    resources: &AtlasResourceView<'_>,
    extents: &[(u32, u32)],
    minimum_area: u64,
    max_dimension: u32,
) -> Result<PackedAtlas, PlatformError> {
    let ordered = ordered_resources(resources);
    for &(width, height) in extents {
        if u64::from(width) * u64::from(height) < minimum_area
            || width > max_dimension
            || height > max_dimension
        {
            continue;
        }
        if let Some(packed) = pack_atlas_for_extent(&ordered, width, height)? {
            return Ok(packed);
        }
    }
    let page_width = MAX_ATLAS_WIDTH.min(max_dimension);
    let page_height = ATLAS_SIDE.min(max_dimension);
    pack_atlas_pages(&ordered, page_width, page_height)?
        .ok_or_else(|| invalid("glyph atlas capacity was exceeded"))
}

fn ordered_resources<'a>(
    resources: &'a AtlasResourceView<'_>,
) -> Vec<(&'a String, &'a AtlasResource)> {
    let mut ordered = resources.iter().collect::<Vec<_>>();
    ordered.sort_by(|(left_id, left), (right_id, right)| {
        right
            .height()
            .cmp(&left.height())
            .then_with(|| right.width().cmp(&left.width()))
            .then_with(|| left_id.cmp(right_id))
    });
    ordered
}

/// Shelf cursor of one page while packing.
struct ShelfCursor {
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    row_height: u32,
    /// Rightmost padded edge of any placed slot.
    used_width: u32,
}

impl ShelfCursor {
    fn new(width: u32, height: u32) -> Self {
        // Columns before `ATLAS_PADDING * 3` hold the reserved white texel.
        Self {
            width,
            height,
            x: ATLAS_PADDING * 3,
            y: ATLAS_PADDING,
            row_height: 0,
            used_width: ATLAS_PADDING * 3,
        }
    }

    /// Places a padded slot on the current row, wrapping once. Returns the
    /// unpadded origin, or `None` without moving the cursor.
    fn place(&mut self, required_width: u32, required_height: u32) -> Option<(u32, u32)> {
        let (mut x, mut y, mut row_height) = (self.x, self.y, self.row_height);
        if x + required_width > self.width {
            x = ATLAS_PADDING * 3;
            y = y.checked_add(row_height + ATLAS_PADDING)?;
            row_height = 0;
        }
        if x + required_width > self.width || y.checked_add(required_height)? > self.height {
            return None;
        }
        self.x = x + required_width;
        self.y = y;
        self.row_height = row_height.max(required_height);
        self.used_width = self.used_width.max(self.x);
        Some((x + ATLAS_PADDING, y + ATLAS_PADDING))
    }

    fn into_allocation(self, width: u32, height: u32) -> AtlasPageAllocation {
        AtlasPageAllocation {
            width,
            height,
            cursor_x: self.x,
            cursor_y: self.y,
            row_height: self.row_height,
            free_slots: Vec::new(),
            freed_area: 0,
        }
    }
}

pub(crate) fn pack_atlas_for_extent(
    ordered: &[(&String, &AtlasResource)],
    width: u32,
    height: u32,
) -> Result<Option<PackedAtlas>, PlatformError> {
    let mut placements = BTreeMap::new();
    let mut cursor = ShelfCursor::new(width, height);
    for &(resource_id, resource) in ordered {
        let Some((x, y)) = cursor.place(
            resource.width() + ATLAS_PADDING * 2,
            resource.height() + ATLAS_PADDING * 2,
        ) else {
            return Ok(None);
        };
        placements.insert(
            resource_id.clone(),
            AtlasPlacement {
                page: 0,
                x,
                y,
                width: resource.width(),
                height: resource.height(),
            },
        );
    }
    Ok(Some(PackedAtlas {
        placements,
        pages: vec![cursor.into_allocation(width, height)],
    }))
}

/// First-fit shelf packing over pages of `page_width` x `page_height`, opening
/// a new page only when no existing page has room. Each page then shrinks to
/// the smallest power-of-two extent that still holds its shelves. Returns
/// `None` when the set needs more than `MAX_ATLAS_PAGES` pages or a single
/// resource exceeds one page.
pub(crate) fn pack_atlas_pages(
    ordered: &[(&String, &AtlasResource)],
    page_width: u32,
    page_height: u32,
) -> Result<Option<PackedAtlas>, PlatformError> {
    let mut placements = BTreeMap::new();
    let mut cursors: Vec<ShelfCursor> = Vec::new();
    for &(resource_id, resource) in ordered {
        let required_width = resource.width() + ATLAS_PADDING * 2;
        let required_height = resource.height() + ATLAS_PADDING * 2;
        let mut placed = cursors.iter_mut().enumerate().find_map(|(page, cursor)| {
            cursor
                .place(required_width, required_height)
                .map(|(x, y)| (page, x, y))
        });
        if placed.is_none() && cursors.len() < MAX_ATLAS_PAGES {
            let mut cursor = ShelfCursor::new(page_width, page_height);
            placed = cursor
                .place(required_width, required_height)
                .map(|(x, y)| (cursors.len(), x, y));
            cursors.push(cursor);
        }
        let Some((page, x, y)) = placed else {
            return Ok(None);
        };
        placements.insert(
            resource_id.clone(),
            AtlasPlacement {
                page,
                x,
                y,
                width: resource.width(),
                height: resource.height(),
            },
        );
    }
    if cursors.is_empty() {
        cursors.push(ShelfCursor::new(page_width, page_height));
    }
    let pages = cursors
        .into_iter()
        .map(|cursor| {
            let width = shrunk_extent(cursor.used_width, page_width);
            let height = shrunk_extent(cursor.y + cursor.row_height, page_height);
            cursor.into_allocation(width, height)
        })
        .collect();
    Ok(Some(PackedAtlas { placements, pages }))
}

fn shrunk_extent(used: u32, maximum: u32) -> u32 {
    used.max(MIN_ATLAS_SIDE)
        .checked_next_power_of_two()
        .map_or(maximum, |extent| extent.min(maximum))
}
