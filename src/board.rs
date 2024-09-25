#[derive(Clone)]
pub struct Board {
    pub spaces: [[Space; 11]; 11],
}

impl Board {
    pub fn new() -> Self {
        let board = Board {
            spaces: [[Space::Empty; 11]; 11],
        };
        // TODO place rocks and daimyos
        board
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Space {
    Empty,
    Occupied(Piece),
}

