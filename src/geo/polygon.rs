use super::{Point, WkbReader};

#[derive(Debug, PartialEq)]
pub(crate) struct Polygon {
    pub(crate) rings: Vec<Vec<Point>>,
}

impl Polygon {
    pub(super) fn read(r: &mut WkbReader) -> Polygon {
        let n_rings = r.read_u32();

        let rings = (0..n_rings).map(|_| Point::read_many(r)).collect();

        Polygon { rings }
    }
}
