#[derive(Clone, Copy, PartialEq)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Clone, Copy)]
pub struct SortState {
    col: usize,
    dir: SortDir,
}

impl SortState {
    pub fn col(&self) -> usize {
        self.col
    }

    pub fn dir(&self) -> SortDir {
        self.dir
    }

    pub fn next(current: Option<SortState>, col: usize) -> Option<SortState> {
        match current {
            Some(s) if s.col == col && s.dir == SortDir::Asc => Some(SortState {
                col,
                dir: SortDir::Desc,
            }),
            Some(s) if s.col == col && s.dir == SortDir::Desc => None,
            _ => Some(SortState {
                col,
                dir: SortDir::Asc,
            }),
        }
    }
}
