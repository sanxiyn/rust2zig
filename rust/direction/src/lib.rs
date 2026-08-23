#[derive(PartialEq, Debug)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

pub fn opposite(d: Direction) -> Direction {
    match d {
        Direction::North => Direction::South,
        Direction::East => Direction::West,
        Direction::South => Direction::North,
        Direction::West => Direction::East,
    }
}

pub fn vertical(d: Direction) -> bool {
    match d {
        Direction::North | Direction::South => true,
        Direction::East | Direction::West => false,
    }
}

#[test]
fn test_opposite() {
    assert_eq!(Direction::South, opposite(Direction::North));
    assert_eq!(Direction::West, opposite(Direction::East));
    assert_eq!(Direction::North, opposite(Direction::South));
    assert_eq!(Direction::East, opposite(Direction::West));
}

#[test]
fn test_vertical() {
    assert_eq!(true, vertical(Direction::North));
    assert_eq!(false, vertical(Direction::East));
    assert_eq!(true, vertical(Direction::South));
    assert_eq!(false, vertical(Direction::West));
}
