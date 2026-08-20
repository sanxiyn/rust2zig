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

#[test]
fn test_direction() {
    assert_eq!(Direction::South, opposite(Direction::North));
    assert_eq!(Direction::West, opposite(Direction::East));
    assert_eq!(Direction::North, opposite(Direction::South));
    assert_eq!(Direction::East, opposite(Direction::West));
}
