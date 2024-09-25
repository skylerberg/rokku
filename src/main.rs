extern crate strum;
extern crate strum_macros;

use std::fmt;
use std::ops;

use mcts::MonteCarloTreeSearch;
use mcts::Status;
use mcts::VanillaMcts;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;


use mcts::Game as MctsGame;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Space {
    Empty,
    Occupied(Piece),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Piece {
    GoodRock,
    GoodRock2,
    BadRock,
    Token(Color, Token),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub fn opposite(&self) -> Self {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, EnumIter)]
pub enum Token {
    Daimyo,
    Scout,
    Hammer,
    Hook,
    Wave,
    Hand,
    Bomb,
}

#[derive(Clone)]
pub enum TurnState {
    WhiteFirstAction,
    WhiteSecondAction { used_piece: Option<Token> },
    BlackFirstAction,
    BlackSecondAction { used_piece: Option<Token> },
    WonBy(Color),
}

impl TurnState {
    // TODO This is a terrible name. How can I explain what I'm doing here?
    pub fn get_matchable(&self) -> Option<(Color, Option<Token>)> {
        match self {
            TurnState::WhiteFirstAction => Some((Color::White, None)),
            TurnState::WhiteSecondAction { used_piece } => Some((Color::White, *used_piece)),
            TurnState::BlackFirstAction => Some((Color::Black, None)),
            TurnState::BlackSecondAction { used_piece } => Some((Color::Black, *used_piece)),
            TurnState::WonBy(_) => None,
        }
    }

    pub fn get_color(&self) -> Option<Color> {
        if let Some((color, _)) = self.get_matchable() {
            Some(color)
        }
        else {
            None
        }
    }
}

impl mcts::Outcome<Color> for TurnState {
    fn reward_for(&self, color: Color) -> f64 {
        match self {
            TurnState::WonBy(winner) => if *winner == color { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct Coordinates(i8, i8, i8);

impl Coordinates {
    pub fn is_off_board(&self) -> bool {
        self.0 < -5
            || self.0 > 5
            || self.1 < -5
            || self.1 > 5
            || self.2 < -5
            || self.2 > 5
    }
}

impl ops::Add<Coordinates> for Coordinates {
    type Output = Coordinates;

    fn add(self, rhs: Coordinates) -> Coordinates {
        Coordinates(self.0 + rhs.0, self.1 + rhs.1, self.2 + rhs.2)
    }
}

impl ops::Add<Direction> for Coordinates {
    type Output = Coordinates;

    fn add(self, rhs: Direction) -> Coordinates {
        let Coordinates(q, r, s) = self;
        match rhs {
            Direction::Left => Coordinates(q - 1, r, s + 1),
            Direction::UpLeft => Coordinates(q, r - 1, s + 1),
            Direction::UpRight => Coordinates(q + 1, r - 1, s),
            Direction::Right => Coordinates(q + 1, r, s - 1),
            Direction::DownRight => Coordinates(q, r + 1, s - 1),
            Direction::DownLeft => Coordinates(q - 1, r + 1, s),
        }
    }
}

#[derive(Clone)]
pub struct Board {
    pub spaces: [[Space; 11]; 11],
}

impl Board {
    pub fn new() -> Self {
        let mut board = Board {
            spaces: [[Space::Empty; 11]; 11],
        };
        board.set_space(Coordinates(0, 0, 0), Space::Occupied(Piece::BadRock));
        board.set_space(Coordinates(0, -4, 4), Space::Occupied(Piece::GoodRock));
        board.set_space(Coordinates(0, 4, -4), Space::Occupied(Piece::GoodRock2));
        board.set_space(Coordinates(2, -1, -1), Space::Occupied(Piece::Token(Color::White, Token::Daimyo)));
        board.set_space(Coordinates(-2, 1, 1), Space::Occupied(Piece::Token(Color::Black, Token::Daimyo)));
        board
    }

    pub fn get_space(&self, coord: Coordinates) -> Space {
        if coord.2 != -coord.0 - coord.1 {
            panic!("Tried to access space that does not exist");
        }
        self.spaces[(coord.0 + 5) as usize][(coord.1 + 5) as usize]
    }

    pub fn set_space(&mut self, coord: Coordinates, space: Space) {
        if coord.2 != -coord.0 - coord.1 {
            panic!("Tried to access space that does not exist");
        }
        self.spaces[(coord.0 + 5) as usize][(coord.1 + 5) as usize] = space;
    }

    pub fn move_to(&mut self, target: Coordinates, destination: Coordinates) {
        self.set_space(destination, self.get_space(target));
        self.set_space(target, Space::Empty);
    }

    pub fn move_piece(&mut self, piece: Piece, direction: Direction) {
        let coordinates = self.find(piece).unwrap();
        self.move_to(coordinates, coordinates + direction);
    }

    fn gate_coords(&self, color: Color) -> Coordinates {
        match color {
            Color::Black => Coordinates(-4, 2, 2),
            Color::White => Coordinates(4, -2, -2),
        }
    }

    pub fn gate_is_empty(&self, color: Color) -> bool {
        let coordinates = self.gate_coords(color);
        self.get_space(coordinates) == Space::Empty
    }

    pub fn deploy(&mut self, color: Color, token: Token) {
        if !self.gate_is_empty(color) {
            panic!("Attempted to deploy to occupied gate");
        }
        self.set_space(self.gate_coords(color), Space::Occupied(Piece::Token(color, token)));
    }

    pub fn find(&self, piece: Piece) -> Option<Coordinates> {
        for q in -5..=5 {
            for r in -5..=5 {
                let s = -q - r;
                match self.get_space(Coordinates(q, r, s)) {
                    Space::Occupied(piece_on_space) => {
                        if piece == piece_on_space {
                            return Some(Coordinates(q, r, s));
                        }
                    }
                    Space::Empty => {}
                }
            }
        }
        None
    }

    pub fn is_in_village(&self, coordinates: Coordinates, color: Color) -> bool {
        match color {
            Color::Black => coordinates.0 <= -4,
            Color::White => coordinates.0 >= 4,
        }
    }

    fn is_empty(&self, coordinates: Coordinates) -> bool {
        matches!(self.get_space(coordinates), Space::Empty)
    }

    fn print(&self) {
        for q in -5..=5 {
            for r in -5..=5 {
                let s = -q - r;
                let coordinates = Coordinates(q, r, s);
                match self.get_space(coordinates) {
                    Space::Occupied(piece) => {
                        println!("{:?} - {:?}", coordinates, piece);
                    }
                    Space::Empty => {}
                }
            }
        }
    }

    fn swap(&mut self, target: Coordinates, destination: Coordinates) {
        let destination_contents = self.get_space(destination);
        self.set_space(destination, self.get_space(target));
        self.set_space(target, destination_contents);
    }

}

#[derive(Clone)]
pub struct Game {
    pub board: Board,
    pub supplies: [Vec<Token>; 2],  // [white_supply, black_supply]
    pub hand_directions: [Direction; 2],  // [white_hand, black_hand]
    pub turn_state: TurnState,
}

impl Game {
    pub fn new() -> Self {
        Game {
            board: Board::new(),
            supplies: [
                vec![Token::Scout, Token::Hammer, Token::Hook, Token::Wave, Token::Hand, Token::Bomb],
                vec![Token::Scout, Token::Hammer, Token::Hook, Token::Wave, Token::Hand, Token::Bomb],
            ],
            hand_directions: [Direction::Left, Direction::Right],
            turn_state: TurnState::WhiteFirstAction,
        }
    }

    fn push(&mut self, target: Coordinates, direction: Direction, distance: usize) {
        let current_position = target;
        let next_position = target + direction;
        for _ in 0..distance {
            if next_position.is_off_board() {
                match self.board.get_space(target) {
                    Space::Occupied(Piece::GoodRock) => {},
                    Space::Occupied(Piece::GoodRock2) => {},
                    Space::Occupied(Piece::BadRock) => {},
                    Space::Occupied(Piece::Token(color, token)) => {
                        self.supplies[color as usize].push(token);
                        self.board.set_space(current_position, Space::Empty);
                    },
                    Space::Empty => panic!("Attempted to push empty space"),
                }
                break;
            }

            if self.board.is_empty(next_position) {
                self.board.move_to(current_position, next_position);
            }
        }
    }

    fn cascading_push(&mut self, target: Coordinates, direction: Direction, distance: usize) {
        let next_position = target + direction;
        if !next_position.is_off_board() && !self.board.is_empty(next_position) {
            self.cascading_push(next_position, direction, distance);
        }
        if !target.is_off_board() && !self.board.is_empty(target) {
            self.push(target, direction, distance);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Choice {
    Pass,
    Deploy(Token),
    Move(Token, Direction),
    UseAbility(Ability),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Ability {
    Hammer { target: Coordinates, direction: Direction, distance: usize },
    Wave { target: Coordinates, destination: Coordinates },
    Scout { target: Coordinates, destination: Coordinates },
    Daimyo { target: Coordinates, destination: Coordinates },
    Hook { target: Coordinates, direction: Direction, distance: usize },
    Bomb { origin: Coordinates },
    Hand { origin: Coordinates, move_direction: Direction, hand_direction: Direction }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, EnumIter)]
pub enum Direction {
    Left,
    UpLeft,
    UpRight,
    Right,
    DownRight,
    DownLeft,
}

impl Direction {
    pub fn opposite(&self) -> Direction  {
        match self {
            Direction::Left => Direction::Right,
            Direction::UpLeft => Direction::DownRight,
            Direction::UpRight => Direction::DownLeft,
            Direction::Right => Direction::Left,
            Direction::DownRight => Direction::UpLeft,
            Direction::DownLeft => Direction::UpRight,
        }
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Choice::Pass => write!(f, "Pass"),
            Choice::Deploy(token) => write!(f, "Deploy {:?}", token),
            Choice::Move(token, direction) => write!(f, "Move {:?} {:?}", token, direction),
            Choice::UseAbility(ability) => write!(f, "Use ability {:?}", ability),
        }
    }
}

impl Default for Choice {
    fn default() -> Self {
        Choice::Pass
    }
}

impl mcts::Game for Game {
    type Choice = Choice;

    type PlayerId = Color;

    type Outcome = TurnState;

    fn get_all_choices(&self) -> Vec<Self::Choice> {
        let mut choices = vec![Choice::Pass];
        match self.turn_state.get_matchable() {
            Some((color, used_piece)) => {
                let mut grabbed_space = None;
                if let Some(hand_space) = self.board.find(Piece::Token(color.opposite(), Token::Hand)) {
                    grabbed_space = Some(hand_space + self.hand_directions[color.opposite() as usize]);
                }

                for token in Token::iter() {
                    if used_piece == Some(token) {
                        continue
                    }

                    if let Some(coordinates) = self.board.find(Piece::Token(color, token)) {
                        if let Some(space) = grabbed_space {
                            if coordinates == space {
                                continue;
                            }
                        }

                        // Hand's move is implement as an ability
                        if token != Token::Hand {
                            // Basic move
                            for direction in Direction::iter() {
                                let new_coordinates = coordinates + direction;
                                if !new_coordinates.is_off_board() && self.board.is_empty(new_coordinates) {
                                    choices.push(Choice::Move(token, direction));
                                }
                            }
                        }

                        // Abilities
                        match token {
                            Token::Daimyo => {
                                for other_token in Token::iter() {
                                    if other_token != Token::Daimyo && !self.supplies[color as usize].contains(&other_token) {
                                        let target = self.board.find(Piece::Token(color, other_token)).unwrap();

                                        if let Some(space) = grabbed_space {
                                            if target == space {
                                                continue;
                                            }
                                        }

                                        for direction in Direction::iter() {
                                            let destination = coordinates + direction;
                                            if !destination.is_off_board() && self.board.is_empty(coordinates + direction) {
                                                choices.push(Choice::UseAbility(Ability::Daimyo { target, destination }));
                                            }
                                        }
                                    }
                                }
                            },
                            Token::Scout => {
                                for direction in Direction::iter() {
                                    let mut destination = coordinates + direction ;
                                    if !destination.is_off_board() && !self.board.is_empty(destination) {
                                        choices.push(Choice::UseAbility(Ability::Scout{target: coordinates, destination}));
                                    }
                                    destination = destination + direction;
                                    if !destination.is_off_board() && !self.board.is_empty(destination) {
                                        choices.push(Choice::UseAbility(Ability::Scout{target: coordinates, destination}));
                                    }
                                }
                            },
                            Token::Hammer => {
                                for direction in Direction::iter() {
                                    let target = coordinates + direction;
                                    if !target.is_off_board() && !self.board.is_empty(target) {
                                        for distance in 1..=3 {
                                            choices.push(Choice::UseAbility(Ability::Hammer{target, direction, distance}));
                                        }
                                    }
                                }
                            },
                            Token::Hook => {
                                for direction in Direction::iter() {
                                    for distance_to_target in 2..=4 {
                                        let mut target = coordinates;
                                        for _ in 0..distance_to_target {
                                            target = target + direction;
                                        }
                                        if !target.is_off_board() && !self.board.is_empty(target) {
                                            for distance_to_pull in 1..distance_to_target {
                                                choices.push(Choice::UseAbility(Ability::Hook {
                                                    target,
                                                    direction: direction.opposite(),
                                                    distance: distance_to_pull,
                                                }));
                                            }
                                        }
                                    }
                                }
                            },
                            Token::Wave => {
                                // TODO implement buffed version of Wave
                                for direction in Direction::iter() {
                                    let target = coordinates + direction;
                                    if !target.is_off_board() && !self.board.is_empty(target) {
                                        let destination = coordinates + direction.opposite();
                                        if !destination.is_off_board() && self.board.is_empty(destination) {
                                            choices.push(Choice::UseAbility(Ability::Wave{target, destination}));
                                        }
                                    }
                                }
                            },
                            Token::Hand => {
                                for move_direction in Direction::iter() {
                                    let new_coordinates = coordinates + move_direction;
                                    if !new_coordinates.is_off_board() && self.board.is_empty(new_coordinates) {
                                        for hand_direction in Direction::iter() {
                                            choices.push(Choice::UseAbility(Ability::Hand{origin: coordinates, move_direction, hand_direction}));
                                        }
                                    }
                                }
                            },
                            Token::Bomb => {
                                choices.push(Choice::UseAbility(Ability::Bomb{ origin: coordinates }));
                            },
                        }
                    }
                }
                if self.board.gate_is_empty(color) {
                    for token in self.supplies[color as usize].iter() {
                        choices.push(Choice::Deploy(*token));
                    }
                }
            },
            None => {},
        }
        choices
    }

    fn apply_choice(&mut self, choice: &Self::Choice) {
        let mut used_piece = None;
        match self.turn_state.get_matchable() {
            Some((color, _)) => {
                match choice {
                    Choice::Pass => {},
                    Choice::Deploy(token) => {
                        self.supplies[color as usize].retain(|&t| t != *token);
                        self.board.deploy(color, *token);
                    },
                    Choice::Move(token, direction) => {
                        self.board.move_piece(Piece::Token(color, *token), *direction);
                        used_piece = Some(*token);
                    },
                    Choice::UseAbility(ability) => {
                        match ability {
                            Ability::Hammer { target, direction, distance } => {
                                used_piece = Some(Token::Hammer);
                                self.push(*target, *direction, *distance);
                            }
                            Ability::Wave { target, destination } => {
                                used_piece = Some(Token::Wave);
                                self.board.move_to(*target, *destination);
                            }
                            Ability::Scout { target, destination } => {
                                used_piece = Some(Token::Scout);
                                self.board.swap(*target, *destination);
                            }
                            Ability::Daimyo { target, destination } => {
                                used_piece = Some(Token::Daimyo);
                                self.board.move_to(*target, *destination);
                            }
                            Ability::Hook { target, direction, distance } => {
                                used_piece = Some(Token::Hook);
                                self.push(*target, direction.opposite(), *distance);
                            }
                            Ability::Bomb { origin } => {
                                used_piece = Some(Token::Bomb);
                                for direction in Direction::iter() {
                                    self.cascading_push(*origin + direction, direction, 1);
                                }
                            }
                            Ability::Hand { origin, move_direction, hand_direction } => {
                                used_piece = Some(Token::Hand);
                                self.board.move_to(*origin, *origin + *move_direction);
                                self.hand_directions[color as usize] = *hand_direction;
                            }
                        }
                    }
                }
            }
            None => {},
        }

        let bad_rock_coordinates = self.board.find(Piece::BadRock).unwrap();
        let good_rock_coordinates = self.board.find(Piece::GoodRock).unwrap();
        let good_rock_2_coordinates = self.board.find(Piece::GoodRock2).unwrap();

        if self.board.is_in_village(bad_rock_coordinates, Color::White) {
            self.turn_state = TurnState::WonBy(Color::Black);
        }
        if self.board.is_in_village(bad_rock_coordinates, Color::Black) {
            self.turn_state = TurnState::WonBy(Color::White);
        }

        if self.board.is_in_village(good_rock_coordinates, Color::White) &&
            self.board.is_in_village(good_rock_2_coordinates, Color::White)
        {
            self.turn_state = TurnState::WonBy(Color::White);
        }
        if self.board.is_in_village(good_rock_coordinates, Color::Black) &&
            self.board.is_in_village(good_rock_2_coordinates, Color::Black)
        {
            self.turn_state = TurnState::WonBy(Color::Black);
        }

        match self.turn_state {
            TurnState::WhiteFirstAction => self.turn_state = TurnState::WhiteSecondAction { used_piece, },
            TurnState::WhiteSecondAction {..} => self.turn_state = TurnState::BlackFirstAction,
            TurnState::BlackFirstAction => self.turn_state = TurnState::BlackSecondAction { used_piece, },
            TurnState::BlackSecondAction {..} => self.turn_state = TurnState::WhiteFirstAction,
            TurnState::WonBy(_) => {},
        }

    }

    fn status(&self) -> mcts::Status<Self::PlayerId, Self::Outcome> {
        match self.turn_state {
            TurnState::WhiteFirstAction | TurnState::WhiteSecondAction {..} => mcts::Status::AwaitingAction(Color::White),
            TurnState::BlackFirstAction | TurnState::BlackSecondAction {..} => mcts::Status::AwaitingAction(Color::Black),
            TurnState::WonBy(_) => mcts::Status::Terminated(self.turn_state.clone())
        }
    }
}

fn main() {
    let iterations = 10000;
    let mut game = Game::new();
    let mut mcts: VanillaMcts<Game> = VanillaMcts::new();
    loop {
        match game.status() {
            Status::AwaitingAction(_) => {
                let (choice, _) = mcts.monte_carlo_tree_search(game.clone(), iterations);

                game.board.print();
                println!("{:?} - {}", game.turn_state.get_color(), choice);

                game.apply_choice(&choice);

                game.board.print();
                println!("------");
            }
            Status::Terminated(_) => {
                return;
            }
        }
    }
}

// Rules questions:
// Is it possible to tie? Same action puts all three rocks in one player's village?
// Can a unit intentionally step off the edge of the map?
// Can Daimyo teleport itself?
// What orientation does Hand deploy in (not clarified in rules)?
// Does Daimyo change hand direction?
