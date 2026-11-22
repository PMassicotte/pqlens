pub(super) struct WkbReader<'a> {
    little_endian: bool,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> WkbReader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        WkbReader {
            little_endian: true,
            bytes,
            pos: 0,
        }
    }

    pub(super) fn read_byte_order(&mut self) {
        self.little_endian = self.bytes[self.pos] == 1;
        self.pos += 1;
    }

    pub(super) fn read_u32(&mut self) -> u32 {
        let arr = self.bytes[self.pos..self.pos + 4].try_into().unwrap();

        let v = if self.little_endian {
            u32::from_le_bytes(arr)
        } else {
            u32::from_be_bytes(arr)
        };

        self.pos += 4;

        v
    }

    pub(super) fn read_f64(&mut self) -> f64 {
        let arr = self.bytes[self.pos..self.pos + 8].try_into().unwrap();

        let v = if self.little_endian {
            f64::from_le_bytes(arr)
        } else {
            f64::from_be_bytes(arr)
        };

        self.pos += 8;

        v
    }
}
