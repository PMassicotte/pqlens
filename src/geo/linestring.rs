use super::{Point, WkbReader};

#[derive(Debug, PartialEq)]
pub(crate) struct LineString {
    pub(crate) points: Vec<Point>,
}

impl LineString {
    pub(super) fn read(r: &mut WkbReader) -> LineString {
        LineString {
            points: Point::read_many(r),
        }
    }
}
