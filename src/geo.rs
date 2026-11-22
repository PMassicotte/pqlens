mod linestring;
mod point;
mod polygon;
mod wkb;

pub(crate) use linestring::LineString;
pub(crate) use point::Point;
pub(crate) use polygon::Polygon;

use wkb::WkbReader;

#[derive(Debug, PartialEq)]
pub(crate) enum Geometry {
    Point(Point),
    LineString(LineString),
    Polygon(Polygon),
}

impl Geometry {
    pub(crate) fn from_wkb(bytes: &[u8]) -> Geometry {
        let mut reader = WkbReader::new(bytes);

        Geometry::read(&mut reader)
    }

    fn read(reader: &mut WkbReader) -> Geometry {
        reader.read_byte_order();

        let ty = reader.read_u32();

        match ty {
            1 => Point::read(reader).into(),
            2 => LineString::read(reader).into(),
            3 => Polygon::read(reader).into(),
            _ => unimplemented!(),
        }
    }
}

impl From<Point> for Geometry {
    fn from(p: Point) -> Self {
        Geometry::Point(p)
    }
}

impl From<LineString> for Geometry {
    fn from(ls: LineString) -> Self {
        Geometry::LineString(ls)
    }
}

impl From<Polygon> for Geometry {
    fn from(poly: Polygon) -> Self {
        Geometry::Polygon(poly)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn parse_simple_point_binary() {
        let hex = "0101000000000000000000F03F0000000000000040";
        let bytes = hex_to_bytes(hex);
        let geometry = Geometry::from_wkb(&bytes);

        assert_eq!(geometry, Geometry::Point(Point { x: 1.0, y: 2.0 }));
    }

    #[test]
    fn parse_simple_linestring_binary() {
        let hex =
            "010200000002000000000000000000F03F000000000000004000000000000008400000000000001040";
        let bytes = hex_to_bytes(hex);
        let geometry = Geometry::from_wkb(&bytes);

        assert_eq!(
            geometry,
            Geometry::LineString(LineString {
                points: vec![Point { x: 1.0, y: 2.0 }, Point { x: 3.0, y: 4.0 }]
            })
        );
    }

    #[test]
    fn parse_simple_polygon_binary() {
        let hex = "010300000002000000050000000000000000000000000000000000000000000000000024400000000000000000000000000000244000000000000024400000000000000000000000000000244000000000000000000000000000000000050000000000000000000040000000000000004000000000000010400000000000000040000000000000104000000000000010400000000000000040000000000000104000000000000000400000000000000040";

        let bytes = hex_to_bytes(hex);
        let geometry = Geometry::from_wkb(&bytes);

        let expected = Geometry::Polygon(Polygon {
            rings: vec![
                // exterior
                vec![
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 10.0, y: 0.0 },
                    Point { x: 10.0, y: 10.0 },
                    Point { x: 0.0, y: 10.0 },
                    Point { x: 0.0, y: 0.0 },
                ],
                // hole
                vec![
                    Point { x: 2.0, y: 2.0 },
                    Point { x: 4.0, y: 2.0 },
                    Point { x: 4.0, y: 4.0 },
                    Point { x: 2.0, y: 4.0 },
                    Point { x: 2.0, y: 2.0 },
                ],
            ],
        });

        assert_eq!(geometry, expected);
    }
}
