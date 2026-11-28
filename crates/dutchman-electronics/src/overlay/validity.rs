//! We identify exact translated copies without changing their input coordinates.
use std::collections::HashSet;

use geo::{Coord, MultiPolygon, Polygon, Validation};

const MAX_SHAPES: usize = 512;
const MAX_POINTS: usize = 256;

/// We use exact binary subtraction: same-sign inputs within a factor of two,
/// or an origin of zero. We otherwise leave validation to the existing path.
fn relative(value: f64, origin: f64) -> Option<u64> {
    if !value.is_finite() || !origin.is_finite() || value.abs() > 1e6 || origin.abs() > 1e6 {
        return None;
    }
    if origin != 0.0
        && ((value > 0.0) != (origin > 0.0)
            || value.abs() < origin.abs() * 0.5
            || value.abs() > origin.abs() * 2.0)
    {
        return None;
    }
    let delta = value - origin;
    Some(if delta == 0.0 { 0 } else { delta.to_bits() })
}

fn key(p: &Polygon) -> Option<Vec<u64>> {
    if p.interiors().len() >= MAX_POINTS {
        return None;
    }
    let Coord { x, y } = *p.exterior().0.first()?;
    let mut out = vec![u64::try_from(p.interiors().len()).ok()?];
    let mut points = 0usize;
    for ring in std::iter::once(p.exterior()).chain(p.interiors()) {
        points = points.checked_add(ring.0.len())?;
        if points > MAX_POINTS {
            return None;
        }
        out.push(u64::try_from(ring.0.len()).ok()?);
        for p in ring.coords() {
            out.push(relative(p.x, x)?);
            out.push(relative(p.y, y)?);
        }
    }
    Some(out)
}

#[derive(Default)]
pub(super) struct Validity {
    valid: HashSet<Vec<u64>>,
}

impl Validity {
    pub(super) fn valid(&mut self, polygon: &Polygon) -> bool {
        let key = key(polygon);
        if key.as_ref().is_some_and(|k| self.valid.contains(k)) {
            return true;
        }
        if !polygon.is_valid() {
            return false;
        }
        if self.valid.len() < MAX_SHAPES {
            if let Some(key) = key {
                self.valid.insert(key);
            }
        }
        true
    }
}

const MANTISSA: u64 = (1u64 << 52) - 1;

// We keep input and output coordinates strictly inside one exponent band. Equal
// origin parity then preserves rounding ties when the engine's centered output
// is translated. Groups outside these conditions use the original union.
fn band(value: f64) -> Option<u64> {
    if !value.is_finite() || value.abs() < 1e-6 || value.abs() > 1e6 {
        return None;
    }
    let bits = value.to_bits();
    let fraction = bits & MANTISSA;
    (2..=(MANTISSA - 2))
        .contains(&fraction)
        .then_some(bits & !MANTISSA)
}

pub(super) struct TranslatedGroup {
    pub(super) origin: Coord,
    pub(super) bands: [u64; 2],
    pub(super) key: Vec<u64>,
}

pub(super) fn group(shape: &MultiPolygon) -> Option<TranslatedGroup> {
    if shape.0.len() > 32 {
        return None;
    }
    let origin = *shape.0.first()?.exterior().0.first()?;
    let bands = [band(origin.x)?, band(origin.y)?];
    let mut key = vec![
        band(origin.x)?,
        band(origin.y)?,
        u64::try_from(shape.0.len()).ok()?,
    ];
    let mut points = 0usize;
    let mut rings = 0usize;
    for p in shape {
        if p.interiors().len() >= MAX_POINTS {
            return None;
        }
        key.push(u64::try_from(p.interiors().len()).ok()?);
        for ring in std::iter::once(p.exterior()).chain(p.interiors()) {
            rings = rings.checked_add(1)?;
            points = points.checked_add(ring.0.len())?;
            if points > MAX_POINTS || rings > MAX_POINTS {
                return None;
            }
            key.push(u64::try_from(ring.0.len()).ok()?);
            for p in ring.coords() {
                if !in_band(*p, bands) {
                    return None;
                }
                key.push(relative(p.x, origin.x)?);
                key.push(relative(p.y, origin.y)?);
            }
        }
    }
    Some(TranslatedGroup { origin, bands, key })
}

pub(super) fn offset(to: Coord, from: Coord) -> Option<Coord> {
    if (to.x.to_bits() & 1) != (from.x.to_bits() & 1)
        || (to.y.to_bits() & 1) != (from.y.to_bits() & 1)
    {
        return None;
    }
    Some(Coord {
        x: f64::from_bits(relative(to.x, from.x)?),
        y: f64::from_bits(relative(to.y, from.y)?),
    })
}

pub(super) fn in_band(p: Coord, bands: [u64; 2]) -> bool {
    band(p.x) == bands.first().copied() && band(p.y) == bands.get(1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{LineString, MapCoords};

    fn polygon(points: &[(f64, f64)]) -> Polygon {
        Polygon::new(LineString::from(points.to_vec()), vec![])
    }

    #[test]
    fn translated_valid_and_crossing_rings_match_the_independent_validator() {
        let shapes = [
            polygon(&[(2.0, 2.0), (3.0, 2.0), (3.0, 3.0), (2.0, 3.0), (2.0, 2.0)]),
            polygon(&[(2.0, 2.0), (3.0, 3.0), (2.0, 3.0), (3.0, 2.0), (2.0, 2.0)]),
            Polygon::new(
                LineString::from(vec![
                    (2.0, 2.0),
                    (3.0, 2.0),
                    (3.0, 3.0),
                    (2.0, 3.0),
                    (2.0, 2.0),
                ]),
                vec![LineString::from(vec![
                    (2.25, 2.25),
                    (2.75, 2.25),
                    (2.75, 2.75),
                    (2.25, 2.75),
                    (2.25, 2.25),
                ])],
            ),
        ];
        let mut cache = Validity::default();
        for shift in [0.0, 8.0, 128.0, 512.0, -16.0] {
            for p in &shapes {
                let shifted = p.map_coords(|p| Coord {
                    x: p.x + shift,
                    y: p.y + shift,
                });
                assert_eq!(cache.valid(&shifted), shifted.is_valid());
            }
        }
        assert!(cache.valid.len() <= shapes.len());
    }

    #[test]
    fn inexact_subtraction_and_large_shapes_use_the_original_validator() {
        assert!(relative(1.0, 1000.0).is_none());
        let p = polygon(&[
            (0.001, 0.001),
            (1000.0, 0.001),
            (1000.0, 1000.0),
            (0.001, 1000.0),
            (0.001, 0.001),
        ]);
        assert!(key(&p).is_none());
        let mut cache = Validity::default();
        assert_eq!(cache.valid(&p), p.is_valid());
        assert!(cache.valid.is_empty());
        let empty_holes = Polygon::new(
            p.exterior().clone(),
            vec![LineString::new(vec![]); MAX_POINTS],
        );
        assert!(key(&empty_holes).is_none());
    }

    #[test]
    fn distinct_shape_count_is_bounded() {
        let mut cache = Validity::default();
        for i in 1..600 {
            let width = 0.1 + f64::from(i) * 0.001;
            let p = polygon(&[
                (2.0, 2.0),
                (2.0 + width, 2.0),
                (2.0 + width, 2.5),
                (2.0, 2.5),
                (2.0, 2.0),
            ]);
            assert!(cache.valid(&p));
        }
        assert!(cache.valid.len() <= MAX_SHAPES);
    }
}
