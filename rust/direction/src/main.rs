enum Direction {
    North,
    East,
    South,
    West,
}

fn to_string(d: Direction) -> &'static str {
    match d {
        Direction::North => "North",
        Direction::East => "East",
        Direction::South => "South",
        Direction::West => "West",
    }
}

fn opposite(d: Direction) -> Direction {
    match d {
        Direction::North => Direction::South,
        Direction::East => Direction::West,
        Direction::South => Direction::North,
        Direction::West => Direction::East,
    }
}

fn main() {
    println!("{}", to_string(opposite(Direction::North)));
    println!("{}", to_string(opposite(Direction::East)));
    println!("{}", to_string(opposite(Direction::South)));
    println!("{}", to_string(opposite(Direction::West)));
}
