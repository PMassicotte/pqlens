use super::WkbReader;

#[derive(Debug, PartialEq)]
pub(crate) struct Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

impl Point {
    pub(super) fn read(r: &mut WkbReader) -> Point {
        Point {
            x: r.read_f64(),
            y: r.read_f64(),
        }
    }

    pub(super) fn read_many(r: &mut WkbReader) -> Vec<Point> {
        let n_pts = r.read_u32();

        (0..n_pts).map(|_| Point::read(r)).collect()
    }
}
