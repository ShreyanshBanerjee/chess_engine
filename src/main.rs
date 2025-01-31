use chess::{
    Board,
    BoardStatus,
    MoveGen,
    Piece,
    Color,
    ChessMove,
    BitBoard
};

use std::collections::HashMap;
use std::io;
use std::str::FromStr;
use std::time::Instant;

const PAWN_PST: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0
];

const KNIGHT_PST: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50,
    -40,-20,  0,  0,  0,  0,-20,-40,
    -30,  0, 10, 15, 15, 10,  0,-30,
    -30,  5, 15, 20, 20, 15,  5,-30,
    -30,  0, 15, 20, 20, 15,  0,-30,
    -30,  5, 10, 15, 15, 10,  5,-30,
    -40,-20,  0,  5,  5,  0,-20,-40,
    -50,-40,-30,-30,-30,-30,-40,-50,
];

const BISHOP_PST: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5, 10, 10,  5,  0,-10,
    -10,  5,  5, 10, 10,  5,  5,-10,
    -10,  0, 10, 10, 10, 10,  0,-10,
    -10, 10, 10, 10, 10, 10, 10,-10,
    -10,  5,  0,  0,  0,  0,  5,-10,
    -20,-10,-10,-10,-10,-10,-10,-20,
];

const ROOK_PST: [i32; 64] = [
      0,  0,  0,  0,  0,  0,  0,  0,
      5, 10, 10, 10, 10, 10, 10,  5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
     -5,  0,  0,  0,  0,  0,  0, -5,
      0,  0,  0,  5,  5,  0,  0,  0
];

const QUEEN_PST: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20,
    -10,  0,  0,  0,  0,  0,  0,-10,
    -10,  0,  5,  5,  5,  5,  0,-10,
     -5,  0,  5,  5,  5,  5,  0, -5,
      0,  0,  5,  5,  5,  5,  0, -5,
    -10,  5,  5,  5,  5,  5,  0,-10,
    -10,  0,  5,  0,  0,  0,  0,-10,
    -20,-10,-10, -5, -5,-10,-10,-20
];

const KING_PST: [i32; 64] = [
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -30,-40,-40,-50,-50,-40,-40,-30,
    -20,-30,-30,-40,-40,-30,-30,-20,
    -10,-20,-20,-20,-20,-20,-20,-10,
     20, 20,  0,  0,  0,  0, 20, 20,
     20, 30, 10,  0,  0, 10, 30, 20
];

const NULL_MOVE_REDUCTION: i32 = 3; //how much to reduce search depth by when doing a null move search
const LATE_MOVE_REDUCTION: i32 = 2; //how much to reduce search by on a late move
const LATE_MOVE_THRESHOLD: usize = 4; //how many moves to search before applying lmp
const DEFAULT_SEARCH_DEPTH: i32 = 8; //will always be achievable <5s

fn query_pst(bitboard: BitBoard, default_val: i32, pst: [i32; 64], white: bool) -> i32 {
    let squares: Vec<_> = (0..64).into_iter().collect();
    let mut val = 0;
    let bb_usize = bitboard.to_size(0);
    
    for index in squares {
        if (bb_usize & (1 << index)) != 0 { //if the bit is 1
            val += default_val + if white { pst[63-index] } else { pst[index] }
        }
    }

    val
}

fn query_pst_single(piece: Piece, square: usize, white: bool) -> i32 {
    let index = if white { 63 - square } else { square };
    return match piece {
        Piece::Pawn => PAWN_PST,
        Piece::Knight => KNIGHT_PST,
        Piece::Bishop => BISHOP_PST,
        Piece::Rook => ROOK_PST,
        Piece::Queen => QUEEN_PST,
        Piece::King => KING_PST
    }[index];
}

fn get_piece_value(piece: Piece) -> i32 {
    match piece {
        Piece::Pawn => 100,
        Piece::Knight => 300,
        Piece::Bishop => 325,
        Piece::Rook => 500,
        Piece::Queen => 900,
        _ => 0
    }
}

#[derive(Clone, Copy)]
struct EvalTracker {
    eval: i32
}

impl EvalTracker {
    fn init(board: &Board) -> Self {
        let white = board.side_to_move() == Color::White;
        
        let cur_bb = board.color_combined(board.side_to_move());
        let opp_bb = !cur_bb;
        
        let pawn = board.pieces(Piece::Pawn);
        let knight = board.pieces(Piece::Knight);
        let bishop = board.pieces(Piece::Bishop);
        let rook = board.pieces(Piece::Rook);
        let queen = board.pieces(Piece::Queen);
        let king = board.pieces(Piece::King);
        
        EvalTracker {
            eval: query_pst(cur_bb & pawn, 100, PAWN_PST, white) - query_pst(opp_bb & pawn, 100, PAWN_PST, !white)
                + query_pst(cur_bb & knight, 300, KNIGHT_PST, white) - query_pst(opp_bb & knight, 300, KNIGHT_PST, !white)
                + query_pst(cur_bb & bishop, 325, BISHOP_PST, white) - query_pst(opp_bb & bishop, 325, BISHOP_PST, !white)
                + query_pst(cur_bb & rook, 500, ROOK_PST, white) - query_pst(opp_bb & rook, 500, ROOK_PST, !white)
                + query_pst(cur_bb & queen, 900, QUEEN_PST, white) - query_pst(opp_bb & queen, 900, QUEEN_PST, !white)
                + query_pst(cur_bb & king, 0, KING_PST, white) - query_pst(opp_bb & king, 0, KING_PST, !white)
        }
    }
    
    fn update(&self, board: &Board, c_move: &ChessMove) -> Self {
        let white = board.side_to_move() == Color::White;
        let piece = board.piece_on(c_move.get_source()).unwrap();
        if let Some(dest_piece) = board.piece_on(c_move.get_dest()) {
            return EvalTracker {
                eval : self.eval 
                    + get_piece_value(dest_piece)
                    + query_pst_single(dest_piece, c_move.get_dest().to_int() as usize, !white)
                    + query_pst_single(piece, c_move.get_dest().to_int() as usize, white)
                    - query_pst_single(piece, c_move.get_source().to_int() as usize, white)
            };
        }
        
        EvalTracker {
            eval : self.eval
                + query_pst_single(piece, c_move.get_dest().to_int() as usize, white)
                - query_pst_single(piece, c_move.get_source().to_int() as usize, white)
        }
    }

    fn flip(&self) -> Self {
        EvalTracker {
            eval : -self.eval
        }
    }
}

struct Engine {
    history: [[[i32; 64]; 64]; 2],
    cache: HashMap<u64, (i32, i32)>,
    counter: i32
}

impl Engine {
    fn update_history(&mut self, cm: ChessMove, playing: Color, depth: i32) {
        self.history
            [if playing == Color::White { 0 } else { 1 }]
            [cm.get_source().to_int() as usize]
            [cm.get_dest().to_int() as usize] 
        += depth * depth;
    }

    fn get_history(&self, cm: &ChessMove, playing: Color) -> i32 {
        self.history
            [if playing == Color::White { 0 } else { 1 }]
            [cm.get_source().to_int() as usize]
            [cm.get_dest().to_int() as usize]
    }
    
    fn get_move_ranking(&self, board: &Board, c_move: &ChessMove) -> i32 {
        let victim = board.piece_on(c_move.get_dest());
        let attacker = board.piece_on(c_move.get_source());
        let history = self.get_history(c_move, board.side_to_move());
        
        if let Some(vic) = victim {
            if let Some(att) = attacker {
                return (10*get_piece_value(att) - get_piece_value(vic))*10000+history;
            }
        }
        
        history
    }
    
    fn get_ordered_moves(&self, board: &Board, move_gen: &mut MoveGen) -> Vec<ChessMove> {
        let mut moves: Vec<_> = move_gen
            .map(|m| (self.get_move_ranking(board, &m), m)) // Precompute rankings
            .collect();

        moves.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        
        moves.into_iter().map(|(_, m)| m).collect()
    }

    fn calculate(
        &mut self,
        board: Board,
        eval_tracker: EvalTracker,
        depth: i32,
        alpha: i32,
        beta: i32,
        nmp: bool
    ) -> i32 {
        if let Some(data) = self.cache.get(&board.get_hash()) {
            if data.0 >= depth {
                return data.1;
            }
        }

        self.counter += 1;

        let result = match board.status() {
            BoardStatus::Checkmate => -99999,
            BoardStatus::Stalemate => -150,
            BoardStatus::Ongoing => {
                if depth == 0 {
                    -eval_tracker.eval
                }
                else {
                    let mut moves = MoveGen::new_legal(&board);
                    let mut best_score = -10000;
                    let mut calpha = alpha;
                    let mut c_score;
                    let player = board.side_to_move();
                    
                    //null move heuristic - skip turn and perform a reduced search, if opponent can
                    //get more advantage than in other branches, prune
                    if nmp && board.null_move().is_some() && depth > NULL_MOVE_REDUCTION && moves.len() >= 10 {
                        let v = self.calculate(board, eval_tracker, depth-NULL_MOVE_REDUCTION, -beta, -beta+1, false);
                        if v >= beta {
                            return beta;
                        }
                    }
                    
                    let flipped = eval_tracker.flip();
                    
                    //normal ab pruning
                    for (i, next_move) in self.get_ordered_moves(&board, &mut moves).into_iter().enumerate() {
                        //after the first 7 moves any other moves will likely be non critical due
                        //to proper move ordering
                        if i < LATE_MOVE_THRESHOLD || depth < LATE_MOVE_REDUCTION {
                            c_score = -self.calculate(board.make_move_new(next_move), flipped.update(&board, &next_move), depth-1, -beta, -alpha, nmp);
                        } else {
                            c_score = -self.calculate(board.make_move_new(next_move), flipped.update(&board, &next_move), depth-LATE_MOVE_REDUCTION, -beta, -alpha, nmp);
                        }
                        calpha = calpha.max(c_score);
                        best_score = best_score.max(c_score);

                        if c_score >= beta { 
                            self.update_history(next_move, player, depth);
                            break;
                        }
                    }
                    
                    best_score
                }
            }
        };
    
        self.cache.insert(board.get_hash(), (depth, result));
        result
    }

    fn find_best(
        &mut self,
        board: Board,
        eval_tracker: EvalTracker,
        max_depth: i32
    ) -> (ChessMove, i32) {
        let mut moves = MoveGen::new_legal(&board);
        let player = board.side_to_move();
        
        //moves.sort_by(|a,b| self.get_history(*b, player).cmp(&self.get_history(*a, player)));
        
        let mut alpha = -99999;
        let mut best_move = None;
        let mut c_score;
        
        let flipped = eval_tracker.flip();
        
        for next_move in self.get_ordered_moves(&board, &mut moves) {
            c_score = -self.calculate(
                board.make_move_new(next_move),
                flipped.update(&board, &next_move),
                max_depth-1,
                -999,   
                999, 
                true
            );
            
            if c_score > alpha {
                alpha = c_score;
                best_move = Some(next_move);
                self.update_history(next_move, player, max_depth);
            }
        }

        (best_move.expect("error - search was terminated before best_move was initialized"), alpha)
    }
}

#[derive(Debug)]
enum UCIToken {
    Uci,
    IsReady,
    UciNewGame,
    Position(Option<String>, Vec<String>),
    Go(i32),
    Reset,
    Stop,
    Unknown
}

fn get_uci() -> UCIToken {
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    
    let mut tokens = input.trim().split_whitespace();
    let command = tokens.next();
    
    return match command {
        Some("uci") => UCIToken::Uci,
        Some("isready") => UCIToken::IsReady,
        Some("ucinewgame") => UCIToken::UciNewGame,
        Some("position") => {
            let fen = match tokens.next() {
                Some("startpos") => None,
                Some("fen") => Some(tokens.by_ref().take(6).collect::<Vec<_>>().join(" ")),
                _ => panic!("missing or incorrect argument(s) to 'position' command")
            };
            
            let moves = match tokens.next() {
                Some("moves") => tokens.map(|x| x.to_string()).collect(),
                Some(_) => panic!("incorrect argument(s) to 'position' command"),
                None => Vec::new()
            };
            
            UCIToken::Position(fen, moves)
        },
        Some("go") => {
            match tokens.next() {
                Some("depth") => UCIToken::Go(tokens.next().unwrap().parse::<i32>().unwrap()),
                Some(_) => UCIToken::Go(DEFAULT_SEARCH_DEPTH),
                None => UCIToken::Go(DEFAULT_SEARCH_DEPTH)
            }
        },
        Some("reset") => UCIToken::Reset,
        Some("stop") => UCIToken::Stop,
        Some(_) => UCIToken::Unknown,
        None => panic!("empty line recieved")
    };
}

fn main() {
    let mut oxide_data = Engine {
        cache      : HashMap::new(),
        history    : [[[0; 64]; 64]; 2],
        counter    : 0
    };
    
    let mut engine_pos = Board::default();
    let mut eval = EvalTracker { eval : 0 };

    loop {
        match get_uci() {
            UCIToken::Uci => {
                println!("id name oxide_engine");
                println!("id authour sbanerjee");
                println!("uciok");
            },
            UCIToken::IsReady => println!("readyok"),
            UCIToken::Position(pos, moves) => {
                match pos {
                    Some(fen) => engine_pos = Board::from_str(&fen).expect("INVALID POSITION PROVIDED"),
                    None => engine_pos = Board::default()
                }
                for c_move in moves {
                    engine_pos = engine_pos.make_move_new(ChessMove::from_str(&c_move).expect("INVALID MOVE PROVIDED"));
                }
                eval = EvalTracker::init(&engine_pos);
            },
            UCIToken::Go(depth) => {
                let start = Instant::now();
                let results = oxide_data.find_best(engine_pos, eval, depth);
                println!("bestmove {}", results.0);
                println!(
                    "info depth {} score cp {} nodes {} time {}",
                    depth, results.1, oxide_data.counter, start.elapsed().as_millis()
                );
            },
            UCIToken::Reset => oxide_data = Engine {
                cache      : HashMap::new(),
                history    : [[[0; 64]; 64]; 2],
                counter    : 0
            },
            UCIToken::Stop => {},
            UCIToken::UciNewGame => {}, //nothing that could benefit from a reset at the end of a
            _ => {}                    //game

        };
    }
}
