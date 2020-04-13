use std::fmt;
use std::time::{Duration, Instant};

use crate::position::{Move, Position, Square, Square::*};

pub struct Engine {
    game_tree: Vec<Position>,
    nodes_searched: usize,
    root_node: usize,
    depth: usize,
}

#[derive(PartialEq, Copy, Clone)]
pub enum Evaluation {
    Heuristic(f64),
    MateIn(i32),
}

impl Evaluation {
    pub fn increase_mate_dist(&mut self) {
        if let Evaluation::MateIn(dist) = self {
            *dist += dist.signum();
        }
    }
}

impl fmt::Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Evaluation::Heuristic(eval) => write!(f, "cp {}", (eval * 100.0).round() as i32),
            Evaluation::MateIn(dist) => {
                let half_move_dist = dist - 1 * dist.signum();
                write!(f, "mate {}", half_move_dist / 2 + half_move_dist % 2)
            }
        }
    }
}

impl std::ops::Neg for Evaluation {
    type Output = Evaluation;
    fn neg(self) -> Self::Output {
        match self {
            Evaluation::Heuristic(eval) => Evaluation::Heuristic(-eval),
            Evaluation::MateIn(dist) => Evaluation::MateIn(-dist),
        }
    }
}

impl PartialOrd for Evaluation {
    fn partial_cmp(&self, other: &Evaluation) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Evaluation::MateIn(s), Evaluation::MateIn(o)) => {
                if s.signum() == o.signum() {
                    o.abs().partial_cmp(&s.abs())
                } else {
                    Some(s.cmp(&o))
                }
            }
            (Evaluation::Heuristic(s), Evaluation::Heuristic(o)) => s.partial_cmp(o),
            (Evaluation::MateIn(s), Evaluation::Heuristic(_)) => Some(s.cmp(&0)),
            (Evaluation::Heuristic(_), Evaluation::MateIn(o)) => Some(0.cmp(o)),
        }
    }
}

pub struct SearchOutput {
    pub best_move: Option<Move>,
    pub nodes_searched: usize,
    pub search_time: Duration,
    pub search_depth: usize,
    pub evaluation: Evaluation,
}

impl Engine {
    pub fn new() -> Engine {
        Engine {
            game_tree: vec![Position::new()],
            depth: 0,
            nodes_searched: 0,
            root_node: 0,
        }
    }

    pub fn set_depth(&mut self, new_depth: usize) {
        self.depth = new_depth;
    }

    pub fn set_game_tree(&mut self, move_iter: &mut dyn Iterator<Item = Move>) {
        self.game_tree = vec![Position::new()];
        for next_move in move_iter {
            let mut next_position = self.game_tree.last().unwrap().clone();
            next_position.make_move(next_move);
            self.game_tree.push(next_position);
        }
        self.root_node = self.game_tree.len() - 1;
    }

    pub fn is_root_white_turn(&self) -> bool {
        self.game_tree[self.root_node].is_white_turn()
    }

    pub fn search_game_tree(&mut self) -> SearchOutput {
        self.nodes_searched = 0;

        let mut candidate_moves_buffer = Vec::with_capacity(self.depth);
        for _ in 0..self.depth {
            candidate_moves_buffer.push(Vec::with_capacity(256));
        }

        let start_time = Instant::now();
        let (best_move, evaluation) = self.min_max_search(
            &mut candidate_moves_buffer,
            Evaluation::MateIn(1),
            Evaluation::MateIn(-1),
        );
        let end_time = Instant::now();

        let search_time = end_time - start_time;
        SearchOutput {
            best_move,
            nodes_searched: self.nodes_searched,
            search_depth: self.depth,
            evaluation,
            search_time,
        }
    }

    fn min_max_search(
        &mut self,
        candidate_moves_buffer: &mut [Vec<Move>],
        black_max_eval: Evaluation,
        white_min_eval: Evaluation,
    ) -> (Option<Move>, Evaluation) {
        self.nodes_searched += 1;

        let cur_depth = self.game_tree.len() - self.root_node - 1;

        assert!(cur_depth <= self.depth);

        let is_white_turn = self.game_tree.last().unwrap().is_white_turn();

        if cur_depth == self.depth {
            return (
                None,
                Self::heuristic_evaluation(self.game_tree.last().unwrap().get_squares()),
            );
        }

        let mut num_same_boards = 1;
        for i in self
            .game_tree
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_, pos)| !pos.resets_draw_counters())
            .map(|(i, _)| i)
        {
            if self.game_tree[i - 1] == *self.game_tree.last().unwrap() {
                num_same_boards += 1;
            }
            if self.game_tree.len() - i >= 100 {
                return (None, Evaluation::Heuristic(0.0));
            }
            // cant be reached more than once
            if num_same_boards >= 3 && !self.game_tree[i - 1].can_en_passant() {
                return (None, Evaluation::Heuristic(0.0));
            }
        }

        if self.game_tree.last().unwrap().is_insufficient_material() {
            return (None, Evaluation::Heuristic(0.0));
        }

        candidate_moves_buffer[0].clear();
        self.game_tree
            .last()
            .unwrap()
            .get_candidate_moves(&mut candidate_moves_buffer[0]);

        let (move_buffer, candidate_moves_buffer) =
            candidate_moves_buffer.split_first_mut().unwrap();

        self.game_tree.push(self.game_tree.last().unwrap().clone());

        let mut best_move: (Option<Move>, Evaluation) =
            (None, Evaluation::MateIn(if is_white_turn { -1 } else { 1 }));

        let mut did_move = false;

        'outer: for _ in 0..1 {
            for m in move_buffer.iter() {
                *self.game_tree.last_mut().unwrap() =
                    self.game_tree[self.game_tree.len() - 2].clone();
                if m.is_capture() && self.game_tree.last_mut().unwrap().make_move(*m) {
                    did_move = true;
                    if is_white_turn {
                        let (_, eval) = self.min_max_search(
                            candidate_moves_buffer,
                            black_max_eval,
                            best_move.1,
                        );
                        if eval >= best_move.1 {
                            best_move = (Some(*m), eval);
                        }
                        if eval > black_max_eval {
                            break 'outer;
                        }
                    } else {
                        let (_, eval) = self.min_max_search(
                            candidate_moves_buffer,
                            best_move.1,
                            white_min_eval,
                        );
                        if eval <= best_move.1 {
                            best_move = (Some(*m), eval);
                        }
                        if eval < white_min_eval {
                            break 'outer;
                        }
                    }
                }
            }

            for m in move_buffer.iter() {
                *self.game_tree.last_mut().unwrap() =
                    self.game_tree[self.game_tree.len() - 2].clone();
                if !m.is_capture() && self.game_tree.last_mut().unwrap().make_move(*m) {
                    did_move = true;
                    if is_white_turn {
                        let (_, eval) = self.min_max_search(
                            candidate_moves_buffer,
                            black_max_eval,
                            best_move.1,
                        );
                        if eval >= best_move.1 {
                            best_move = (Some(*m), eval);
                        }
                        if eval > black_max_eval {
                            break 'outer;
                        }
                    } else {
                        let (_, eval) = self.min_max_search(
                            candidate_moves_buffer,
                            best_move.1,
                            white_min_eval,
                        );
                        if eval <= best_move.1 {
                            best_move = (Some(*m), eval);
                        }
                        if eval < white_min_eval {
                            break 'outer;
                        }
                    }
                }
            }
        }

        self.game_tree.pop();

        best_move.1.increase_mate_dist();

        if !did_move {
            return if self.game_tree.last().unwrap().is_check() {
                (None, Evaluation::MateIn(if is_white_turn { -1 } else { 1 }))
            } else {
                (None, Evaluation::Heuristic(0.0))
            };
        }

        best_move
    }

    fn heuristic_evaluation(position: &[Square; 64]) -> Evaluation {
        let mut material_score = 0.0;

        for (i, piece) in position.iter().enumerate() {
            material_score += match piece {
                Empty => 0.0,
                WhitePawn => 1.0 + (i / 8 - 1) as f64 / 80.0,
                BlackPawn => -1.0 - (7 - i / 8) as f64 / 80.0,
                WhiteKnight => 3.0,
                BlackKnight => -3.0,
                WhiteBishop => 3.0,
                BlackBishop => -3.0,
                WhiteRook => 5.0,
                BlackRook => -5.0,
                WhiteKing => 0.0,
                BlackKing => 0.0,
                WhiteQueen => 9.0,
                BlackQueen => -9.0,
            }
        }
        Evaluation::Heuristic(material_score)
    }
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{:?}", self.game_tree[self.root_node])
    }
}
