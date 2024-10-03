extern crate colored;
extern crate strum;
extern crate strum_macros;

use std::fmt;
use std::ops;

use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use colored::Colorize;

use mcts::MonteCarloTreeSearch;
use mcts::VanillaMcts;


use mcts::Game as MctsGame;


const USE_VARIANT_PIT_OF_MISFORTUNE: bool = true;
const USE_VARIANT_PERMA_DEATH: bool = true;
const USE_VARIANT_REVIVE_ACTION: bool = true;
const USE_VARIANT_END_IF_GOOD_ROCK_IN_BOTH_VILLAGES: bool = true;

const CENTER_SPACE: Coordinates = Coordinates(0, 0, 0);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Space {
    Empty,
    Occupied(Piece),
}

impl fmt::Display for Space {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Space::Empty => write!(f, "⚫"),
            Space::Occupied(piece) => write!(f, "{}", piece),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Piece {
    GoodRock,
    GoodRock2,
    BadRock,
    Token(Color, Token),
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Piece::BadRock => write!(f, "💀"),
            Piece::GoodRock | Piece::GoodRock2 => write!(f, "🪨"),
            Piece::Token(color, token) => {
                match color {
                    Color::White => write!(f, "{}", format!("{}", token).on_white()),
                    Color::Black => write!(f, "{}", format!("{}", token).on_black()),
                }
            }
        }
    }
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

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Daimyo => write!(f, "🧙"),
            Token::Scout => write!(f, "🥾"),
            Token::Hammer => write!(f, "🔨"),
            Token::Hook => write!(f, "🪝"),
            Token::Wave => write!(f, "🌊"),
            Token::Hand => write!(f, "🤏"),
            Token::Bomb => write!(f, "💣"),
        }
    }
}

#[derive(Clone)]
pub enum TurnState {
    WhiteFirstAction,
    WhiteSecondAction { used_piece: Option<Token> },
    BlackFirstAction,
    BlackSecondAction { used_piece: Option<Token> },
    WonBy(Option<Color>),
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
        board.set_space(CENTER_SPACE, Space::Occupied(Piece::BadRock));
        board.set_space(Coordinates(-4, 0, 4), Space::Occupied(Piece::GoodRock));
        board.set_space(Coordinates(4, 0, -4), Space::Occupied(Piece::GoodRock2));
        board.set_space(Coordinates(-1, 2, -1), Space::Occupied(Piece::Token(Color::White, Token::Daimyo)));
        board.set_space(Coordinates(1, -2, 1), Space::Occupied(Piece::Token(Color::Black, Token::Daimyo)));
        board
    }

    pub fn get_space(&self, coord: Coordinates) -> Space {
        self.spaces[(coord.0 + 5) as usize][(coord.1 + 5) as usize]
    }

    pub fn set_space(&mut self, coord: Coordinates, space: Space) {
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
            Color::Black => Coordinates(2, -4, 2),
            Color::White => Coordinates(-2, 4, -2),
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
            let r_min = std::cmp::max(-5, -5 - q);
            let r_max = std::cmp::min(5, 5 - q);
            for r in r_min..=r_max {
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

    pub fn find_all_of_color<'a>(&'a self, target_color: Color) -> impl Iterator<Item = (Token, Coordinates)> + 'a {
        (-5..=5).flat_map(move |q| {
            let r_min = std::cmp::max(-5, -5 - q);
            let r_max = std::cmp::min(5, 5 - q);
            (r_min..=r_max).filter_map(move |r| {
                let s = -q - r;
                match self.get_space(Coordinates(q, r, s)) {
                    Space::Occupied(Piece::Token(color, token)) => {
                        if color == target_color {
                            Some((token, Coordinates(q, r, s)))
                        }
                        else {
                            None
                        }
                    }
                    _ => None
                }
            })
        })
    }

    pub fn is_in_village(&self, coordinates: Coordinates, color: Color) -> bool {
        match color {
            Color::Black => coordinates.1 <= -4,
            Color::White => coordinates.1 >= 4,
        }
    }

    fn is_empty(&self, coordinates: Coordinates) -> bool {
        matches!(self.get_space(coordinates), Space::Empty)
    }

    fn print(&self) {
        //for q in -5..=5 {
        //    for r in -5..=5 {
        //        let s = -q - r;
        //        let coordinates = Coordinates(q, r, s);
        //        match self.get_space(coordinates) {
        //            Space::Occupied(piece) => {
        //                println!("{:?} - {:?}", coordinates, piece);
        //            }
        //            Space::Empty => {}
        //        }
        //    }
        //}
        println!(
"
          {0}  {1}  {2}  {3}  {4}  {5}
        {6}  {7}  {8}  {9}  {10}  {11}  {12}
      {13}  {14}  {15}  {16}  {17}  {18}  {19}  {20}
    {21}  {22}  {23}  {24}  {25}  {26}  {27}  {28}  {29}
  {30}  {31}  {32}  {33}  {34}  {35}  {36}  {37}  {38}  {39}
{40}  {41}  {42}  {43}  {44}  {45}  {46}  {47}  {48}  {49}  {50}
  {51}  {52}  {53}  {54}  {55}  {56}  {57}  {58}  {59}  {60}
    {61}  {62}  {63}  {64}  {65}  {66}  {67}  {68}  {69}
      {70}  {71}  {72}  {73}  {74}  {75}  {76}  {77}
        {78}  {79}  {80}  {81}  {82}  {83}  {84}
          {85}  {86}  {87}  {88}  {89}  {90}
",
self.get_space(Coordinates(0, -5, 5)), self.get_space(Coordinates(1, -5, 4)), self.get_space(Coordinates(2, -5, 3)), self.get_space(Coordinates(3, -5, 2)), self.get_space(Coordinates(4, -5, 1)), self.get_space(Coordinates(5, -5, 0)),

self.get_space(Coordinates(-1, -4, 5)), self.get_space(Coordinates(0, -4, 4)), self.get_space(Coordinates(1, -4, 3)), self.get_space(Coordinates(2, -4, 2)), self.get_space(Coordinates(3, -4, 1)), self.get_space(Coordinates(4, -4, 0)), self.get_space(Coordinates(5, -4, -1)),

self.get_space(Coordinates(-2, -3, 5)), self.get_space(Coordinates(-1, -3, 4)), self.get_space(Coordinates(0, -3, 3)), self.get_space(Coordinates(1, -3, 2)), self.get_space(Coordinates(2, -3, 1)), self.get_space(Coordinates(3, -3, 0)), self.get_space(Coordinates(4, -3, -1)), self.get_space(Coordinates(5, -3, -2)),

self.get_space(Coordinates(-3, -2, 5)), self.get_space(Coordinates(-2, -2, 4)), self.get_space(Coordinates(-1, -2, 3)), self.get_space(Coordinates(0, -2, 2)), self.get_space(Coordinates(1, -2, 1)), self.get_space(Coordinates(2, -2, 0)), self.get_space(Coordinates(3, -2, -1)), self.get_space(Coordinates(4, -2, -2)), self.get_space(Coordinates(5, -2, -3)),

self.get_space(Coordinates(-4, -1, 5)), self.get_space(Coordinates(-3, -1, 4)), self.get_space(Coordinates(-2, -1, 3)), self.get_space(Coordinates(-1, -1, 2)), self.get_space(Coordinates(0, -1, 1)), self.get_space(Coordinates(1, -1, 0)), self.get_space(Coordinates(2, -1, -1)), self.get_space(Coordinates(3, -1, -2)), self.get_space(Coordinates(4, -1, -3)), self.get_space(Coordinates(5, -1, -4)),

self.get_space(Coordinates(-5, 0, 5)), self.get_space(Coordinates(-4, 0, 4)), self.get_space(Coordinates(-3, 0, 3)), self.get_space(Coordinates(-2, 0, 2)), self.get_space(Coordinates(-1, 0, 1)), self.get_space(Coordinates(0, 0, 0)), self.get_space(Coordinates(1, 0, -1)), self.get_space(Coordinates(2, 0, -2)), self.get_space(Coordinates(3, 0, -3)), self.get_space(Coordinates(4, 0, -4)), self.get_space(Coordinates(5, 0, -5)),

self.get_space(Coordinates(-5, 1, 4)), self.get_space(Coordinates(-4, 1, 3)), self.get_space(Coordinates(-3, 1, 2)), self.get_space(Coordinates(-2, 1, 1)), self.get_space(Coordinates(-1, 1, 0)), self.get_space(Coordinates(0, 1, -1)), self.get_space(Coordinates(1, 1, -2)), self.get_space(Coordinates(2, 1, -3)), self.get_space(Coordinates(3, 1, -4)), self.get_space(Coordinates(4, 1, -5)),

self.get_space(Coordinates(-5, 2, 3)), self.get_space(Coordinates(-4, 2, 2)), self.get_space(Coordinates(-3, 2, 1)), self.get_space(Coordinates(-2, 2, 0)), self.get_space(Coordinates(-1, 2, -1)), self.get_space(Coordinates(0, 2, -2)), self.get_space(Coordinates(1, 2, -3)), self.get_space(Coordinates(2, 2, -4)), self.get_space(Coordinates(3, 2, -5)),

self.get_space(Coordinates(-5, 3, 2)), self.get_space(Coordinates(-4, 3, 1)), self.get_space(Coordinates(-3, 3, 0)), self.get_space(Coordinates(-2, 3, -1)), self.get_space(Coordinates(-1, 3, -2)), self.get_space(Coordinates(0, 3, -3)), self.get_space(Coordinates(1, 3, -4)), self.get_space(Coordinates(2, 3, -5)),

self.get_space(Coordinates(-5, 4, 1)), self.get_space(Coordinates(-4, 4, 0)), self.get_space(Coordinates(-3, 4, -1)), self.get_space(Coordinates(-2, 4, -2)), self.get_space(Coordinates(-1, 4, -3)), self.get_space(Coordinates(0, 4, -4)), self.get_space(Coordinates(1, 4, -5)),

self.get_space(Coordinates(-5, 5, 0)), self.get_space(Coordinates(-4, 5, -1)), self.get_space(Coordinates(-3, 5, -2)), self.get_space(Coordinates(-2, 5, -3)), self.get_space(Coordinates(-1, 5, -4)), self.get_space(Coordinates(0, 5, -5)),
)

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
    pub graveyards: [Vec<Token>; 2],  // [white_supply, black_supply]
    pub hand_directions: [Direction; 2],  // [white_hand, black_hand]
    pub turn_state: TurnState,
    pub choice_number: u32,
}

impl Game {
    pub fn new() -> Self {
        Game {
            board: Board::new(),
            supplies: [
                vec![Token::Scout, Token::Hammer, Token::Hook, Token::Wave, Token::Hand, Token::Bomb],
                vec![Token::Scout, Token::Hammer, Token::Hook, Token::Wave, Token::Hand, Token::Bomb],
            ],
            graveyards: [
                vec![],
                vec![],
            ],
            hand_directions: [Direction::Left, Direction::Right],
            turn_state: TurnState::WhiteFirstAction,
            choice_number: 0,
        }
    }

    fn push(&mut self, target: Coordinates, direction: Direction, distance: usize) {
        let mut previous_position = target;
        let mut next_position = target + direction;
        for _ in 0..distance {
            if next_position.is_off_board() {
                match self.board.get_space(previous_position) {
                    Space::Occupied(Piece::GoodRock) => {},
                    Space::Occupied(Piece::GoodRock2) => {},
                    Space::Occupied(Piece::BadRock) => {},
                    Space::Occupied(Piece::Token(color, token)) => {
                        if USE_VARIANT_PERMA_DEATH {
                            self.graveyards[color as usize].push(token);
                        }
                        else {
                            self.supplies[color as usize].push(token);
                        }
                        self.board.set_space(previous_position, Space::Empty);
                    },
                    Space::Empty => panic!("Attempted to push empty space"),
                }
                break;
            }

            if self.board.is_empty(next_position) {
                self.board.move_to(previous_position, next_position);
            }
            else {
                break;
            }

            previous_position = next_position;
            next_position = next_position + direction;
        }
    }

    fn cascading_push(&mut self, target: Coordinates, direction: Direction, distance: usize) {
        if !target.is_off_board() && !self.board.is_empty(target) {
            let next_position = target + direction;
            self.cascading_push(next_position, direction, distance);

            self.push(target, direction, distance);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Choice {
    Pass,
    Deploy(Token, Option<Direction>),
    Move(Token, Direction),
    UseAbility(Ability),
    Revive(Token),
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
            Choice::Deploy(token, direction) => write!(f, "Deploy {:?} {:?}", token, direction),
            Choice::Move(token, direction) => write!(f, "Move {:?} {:?}", token, direction),
            Choice::UseAbility(ability) => write!(f, "Use ability {:?}", ability),
            Choice::Revive(token) => write!(f, "Revive {:?}", token),
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

    fn get_all_choices(&self) -> Vec<Self::Choice> {
        let mut choices = Vec::with_capacity(32);
        choices.push(Choice::Pass);
        match self.turn_state.get_matchable() {
            Some((color, used_piece)) => {
                let mut grabbed_space = None;
                if let Some(hand_space) = self.board.find(Piece::Token(color.opposite(), Token::Hand)) {
                    grabbed_space = Some(hand_space + self.hand_directions[color.opposite() as usize]);
                }

                // Use token
                for (token, coordinates) in self.board.find_all_of_color(color) {
                    if used_piece == Some(token) {
                        continue
                    }

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
                            for (other_token, target) in self.board.find_all_of_color(color) {
                                if other_token != Token::Daimyo {
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

                // Deploy
                if self.board.gate_is_empty(color) {
                    for token in self.supplies[color as usize].iter() {
                        if *token == Token::Hand {
                            for direction in Direction::iter() {
                                choices.push(Choice::Deploy(*token, Some(direction)));
                            }
                        }
                        else {
                            choices.push(Choice::Deploy(*token, None));
                        }
                    }
                }

                // Revive
                if USE_VARIANT_REVIVE_ACTION {
                    for token in self.graveyards[color as usize].iter() {
                        choices.push(Choice::Revive(*token));
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
                    Choice::Deploy(token, direction) => {
                        self.supplies[color as usize].retain(|&t| t != *token);
                        self.board.deploy(color, *token);
                        if let Some(direction) = direction {
                            self.hand_directions[color as usize] = *direction;
                        }
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
                                self.push(*target, *direction, *distance);
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
                    Choice::Revive(token) => {
                        self.supplies[color as usize].push(*token);
                        self.graveyards[color as usize].retain(|&t| t != *token);
                    }
                }
            }
            None => {},
        }

        if USE_VARIANT_PIT_OF_MISFORTUNE {
            match self.board.get_space(CENTER_SPACE) {
                Space::Occupied(Piece::Token(color, token)) => {
                    self.board.set_space(CENTER_SPACE, Space::Empty);
                    if USE_VARIANT_PERMA_DEATH {
                        self.graveyards[color as usize].push(token)
                    }
                    else {
                        self.supplies[color as usize].push(token)
                    }
                },
                _ => {},
            }
        }

        let bad_rock_coordinates = self.board.find(Piece::BadRock).unwrap();
        let good_rock_coordinates = self.board.find(Piece::GoodRock).unwrap();
        let good_rock_2_coordinates = self.board.find(Piece::GoodRock2).unwrap();

        if self.board.is_in_village(bad_rock_coordinates, Color::White) {
            self.turn_state = TurnState::WonBy(Some(Color::Black));
        }
        if self.board.is_in_village(bad_rock_coordinates, Color::Black) {
            self.turn_state = TurnState::WonBy(Some(Color::White));
        }

        if self.board.is_in_village(good_rock_coordinates, Color::White) &&
            self.board.is_in_village(good_rock_2_coordinates, Color::White)
        {
            self.turn_state = TurnState::WonBy(Some(Color::White));
        }
        if self.board.is_in_village(good_rock_coordinates, Color::Black) &&
            self.board.is_in_village(good_rock_2_coordinates, Color::Black)
        {
            self.turn_state = TurnState::WonBy(Some(Color::Black));
        }

        if USE_VARIANT_END_IF_GOOD_ROCK_IN_BOTH_VILLAGES {
            if self.board.is_in_village(good_rock_coordinates, Color::Black) &&
                self.board.is_in_village(good_rock_2_coordinates, Color::White) ||
                (self.board.is_in_village(good_rock_coordinates, Color::White) &&
                    self.board.is_in_village(good_rock_2_coordinates, Color::Black))
            {
                if bad_rock_coordinates.1 < 0 {
                    self.turn_state = TurnState::WonBy(Some(Color::White));
                }
                else if bad_rock_coordinates.1 > 0 {
                    self.turn_state = TurnState::WonBy(Some(Color::White));
                }
                // TODO what should happen if the rock is in the middle?
            }
        }

        match self.turn_state {
            TurnState::WhiteFirstAction => self.turn_state = TurnState::WhiteSecondAction { used_piece, },
            TurnState::WhiteSecondAction {..} => self.turn_state = TurnState::BlackFirstAction,
            TurnState::BlackFirstAction => self.turn_state = TurnState::BlackSecondAction { used_piece, },
            TurnState::BlackSecondAction {..} => self.turn_state = TurnState::WhiteFirstAction,
            TurnState::WonBy(_) => {},
        }

        self.choice_number += 1;
    }

    fn heuristic_early_terminate(&self) -> bool {
        if self.choice_number > 50 {
            true
        }
        else {
            false
        }
    }

    fn get_active_player_id(&self) -> Self::PlayerId {
        match self.turn_state {
            TurnState::WhiteFirstAction | TurnState::WhiteSecondAction {..} => Color::White,
            TurnState::BlackFirstAction | TurnState::BlackSecondAction {..} => Color::Black,
            TurnState::WonBy(_) => panic!("Attempted to get active player in terminal game"),
        }
    }

    fn is_terminal(&self) -> bool {
        match self.turn_state {
            TurnState::WhiteFirstAction
                | TurnState::WhiteSecondAction {..}
                | TurnState::BlackFirstAction
                | TurnState::BlackSecondAction {..} => false,
            TurnState::WonBy(_) => true,
        }
    }

    fn reward_for(&self, color: Color) -> f64 {
        match self.turn_state {
            TurnState::WonBy(result) => {
                if let Some(winner) = result {
                    if winner == color { 1.0 } else { 0.0 }
                }
                else {
                    0.5
                }
            }
            _ => 0.0,
        }
    }
}

fn main() {
    let iterations = 1000000;
    let mut game = Game::new();
    let mut mcts: VanillaMcts<Game> = VanillaMcts::new();
    //game.apply_choice(&Choice::Deploy(Token::Hammer, None));
    //game.apply_choice(&Choice::UseAbility(Ability::Daimyo { target: Coordinates(-2, 4, -2), destination: Coordinates(0, 1, -1)}));
    //game.apply_choice(&Choice::Move(Token::Daimyo, Direction::DownLeft));
    //game.apply_choice(&Choice::Deploy(Token::Hand, Some(Direction::DownLeft)));
    //game.apply_choice(&Choice::UseAbility(Ability::Daimyo { target: Coordinates(0, 1, -1), destination: Coordinates(-1, 1, 0)}));
    //game.apply_choice(&Choice::UseAbility(Ability::Hammer { target: Coordinates(0, 0, 0), direction: Direction::UpRight, distance: 2 }));
    //game.apply_choice(&Choice::UseAbility(Ability::Daimyo { target: Coordinates(2, -4, 2), destination: Coordinates(0, 0, 0)}));
    game.board.print();
    println!("------");
    while !game.is_terminal() {
        let (choice, _) = mcts.monte_carlo_tree_search(game.clone(), iterations);
        println!("{:?} - {}", game.turn_state.get_color(), choice);

        game.apply_choice(&choice);

        game.board.print();
        println!("------");
    }
}

// Rules questions:
// Is it possible to tie? Same action puts all three rocks in one player's village?
// Can a unit intentionally step off the edge of the map?
// Can Daimyo teleport itself?
// What orientation does Hand deploy in (not clarified in rules)?
// Does Daimyo change hand direction?
