use pyo3::prelude::*;
use std::sync::OnceLock;
use std::time::Instant;






#[pyclass]
#[derive(Clone, Debug)]
pub struct EvalWeights {
    // Middlegame weights
    #[pyo3(get, set)] pub mg_course_mult: f32,
    #[pyo3(get, set)] pub mg_delta_penalty_mult: f32,
    #[pyo3(get, set)] pub mg_surface_mult: f32,
    #[pyo3(get, set)] pub mg_tunnel_bonus: f32,
    #[pyo3(get, set)] pub mg_immunity_bonus: f32,
    #[pyo3(get, set)] pub mg_wall_base_value: f32,
    #[pyo3(get, set)] pub mg_wall_inflation: f32,
    #[pyo3(get, set)] pub mg_parity_bonus: f32,
    #[pyo3(get, set)] pub mg_scarcity_penalty: f32,
    #[pyo3(get, set)] pub mg_panic_penalty: f32,
    #[pyo3(get, set)] pub mg_tempo_bonus: f32,
    #[pyo3(get, set)] pub mg_gravity_bonus: f32,

    // Endgame weights
    #[pyo3(get, set)] pub eg_course_mult: f32,
    #[pyo3(get, set)] pub eg_delta_penalty_mult: f32,
    #[pyo3(get, set)] pub eg_surface_mult: f32,
    #[pyo3(get, set)] pub eg_tunnel_bonus: f32,
    #[pyo3(get, set)] pub eg_immunity_bonus: f32,
    #[pyo3(get, set)] pub eg_wall_base_value: f32,
    #[pyo3(get, set)] pub eg_wall_inflation: f32,
    #[pyo3(get, set)] pub eg_parity_bonus: f32,
    #[pyo3(get, set)] pub eg_scarcity_penalty: f32,
    #[pyo3(get, set)] pub eg_panic_penalty: f32,
    #[pyo3(get, set)] pub eg_tempo_bonus: f32,
    #[pyo3(get, set)] pub eg_gravity_bonus: f32,
}

#[pymethods]
impl EvalWeights {
    #[new]
    pub fn new() -> Self {
        EvalWeights {
            // Middlegame weights
            mg_course_mult: 6.46,
            mg_delta_penalty_mult: 1.10,
            mg_surface_mult: 0.50,
            mg_tunnel_bonus: 1.90,
            mg_immunity_bonus: 19.47,
            mg_wall_base_value: 3.87,
            mg_wall_inflation: 30.32,
            mg_parity_bonus: 2.26,
            mg_scarcity_penalty: 2.82,
            mg_panic_penalty: 15.08,
            mg_tempo_bonus: 1.75,
            mg_gravity_bonus: 1.00,

            // Endgame weights
            eg_course_mult: 10.10,
            eg_delta_penalty_mult: 2.49,
            eg_surface_mult: 0.00,      // surface is ignored in the endgame
            eg_tunnel_bonus: 2.85,
            eg_immunity_bonus: 28.55,
            eg_wall_base_value: 1.59,
            eg_wall_inflation: 9.93,
            eg_parity_bonus: 0.34,
            eg_scarcity_penalty: 0.06,
            eg_panic_penalty: 5.41,
            eg_tempo_bonus: 0.55,
            eg_gravity_bonus: 1.34,
        }
    }

    #[staticmethod]
    pub fn from_list(w: Vec<f32>) -> Self {
        EvalWeights {
            mg_course_mult: w[0], mg_delta_penalty_mult: w[1], mg_surface_mult: w[2],
            mg_tunnel_bonus: w[3], mg_immunity_bonus: w[4], mg_wall_base_value: w[5],
            mg_wall_inflation: w[6], mg_parity_bonus: w[7], mg_scarcity_penalty: w[8],
            mg_panic_penalty: w[9], mg_tempo_bonus: w[10], mg_gravity_bonus: w[11],

            eg_course_mult: w[12], eg_delta_penalty_mult: w[13], eg_surface_mult: w[14],
            eg_tunnel_bonus: w[15], eg_immunity_bonus: w[16], eg_wall_base_value: w[17],
            eg_wall_inflation: w[18], eg_parity_bonus: w[19], eg_scarcity_penalty: w[20],
            eg_panic_penalty: w[21], eg_tempo_bonus: w[22], eg_gravity_bonus: w[23],
        }
    }
}







// Zobrist hashing table
pub struct ZobristTable {
    pub p0_pos: [u64; 81],
    pub p1_pos: [u64; 81],
    pub walls_h: [u64; 64],
    pub walls_v: [u64; 64],
    pub player_turn: u64, // XORed in only when it's P1's turn
}


impl ZobristTable {
    fn init() -> Self {
        let mut prng = 1070372_u64; // seed

        // xorshift PRNG
        let mut next_rnd = || -> u64 {
            prng ^= prng << 13;
            prng ^= prng >> 7;
            prng ^= prng << 17;
            prng
        };

        let mut table = ZobristTable {
            p0_pos: [0; 81], p1_pos: [0; 81],
            walls_h: [0; 64], walls_v: [0; 64],
            player_turn: next_rnd(),
        };

        for i in 0..81 {
            table.p0_pos[i] = next_rnd();
            table.p1_pos[i] = next_rnd();
        }
        for i in 0..64 {
            table.walls_h[i] = next_rnd();
            table.walls_v[i] = next_rnd();
        }
        table
    }
}


static ZOBRIST: OnceLock<ZobristTable> = OnceLock::new();

#[inline]
pub fn get_zobrist() -> &'static ZobristTable {
    ZOBRIST.get_or_init(ZobristTable::init)
}







#[pyclass]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameState {
    pub positions: [u8; 2],
    pub walls_left: [u8; 2],
    pub player_to_move: u8,
    #[pyo3(get)]
    pub walls_h: u64,
    #[pyo3(get)]
    pub walls_v: u64,
    #[pyo3(get)]
    pub hash: u64, 
}

// =================================================================
//  Internal mechanics (not exposed to Python)
// =================================================================
impl GameState {
    const TOP_EDGE: u128 = 0x1FF;
    const BOTTOM_EDGE: u128 = 0x1FF << 72;
    const LEFT_EDGE: u128 = 1 | (1<<9) | (1<<18) | (1<<27) | (1<<36) | (1<<45) | (1<<54) | (1<<63) | (1<<72);
    const RIGHT_EDGE: u128 = Self::LEFT_EDGE << 8;

    #[inline]
    pub fn xy_to_cell_idx(x: u8, y: u8) -> u8 {
        debug_assert!(x < 9 && y < 9, "Cell ({}, {}) is out of bounds", x, y);
        y * 9 + x
    }

    #[inline]
    pub fn xy_to_wall_idx(x: u8, y: u8) -> u8 {
        debug_assert!(x < 8 && y < 8, "Wall ({}, {}) is out of bounds", x, y);
        y * 8 + x
    }

    #[inline]
    fn expand_8x8_to_9x9(walls: u64) -> u128 {
        let mut res = 0u128;
        res |= walls as u128 & 0xFF;
        res |= ((walls >> 8) as u128 & 0xFF) << 9;
        res |= ((walls >> 16) as u128 & 0xFF) << 18;
        res |= ((walls >> 24) as u128 & 0xFF) << 27;
        res |= ((walls >> 32) as u128 & 0xFF) << 36;
        res |= ((walls >> 40) as u128 & 0xFF) << 45;
        res |= ((walls >> 48) as u128 & 0xFF) << 54;
        res |= ((walls >> 56) as u128 & 0xFF) << 63;
        res
    }

    #[inline]
    fn get_wall_blockers(&self) -> (u128, u128, u128, u128) {
        let w_h = Self::expand_8x8_to_9x9(self.walls_h);
        let w_v = Self::expand_8x8_to_9x9(self.walls_v);

        let block_down_walls = w_h | (w_h << 1);
        let block_down = block_down_walls | Self::BOTTOM_EDGE;
        let block_up = (block_down_walls << 9) | Self::TOP_EDGE;

        let block_right_walls = w_v | (w_v << 9);
        let block_right = block_right_walls | Self::RIGHT_EDGE;
        let block_left = (block_right_walls << 1) | Self::LEFT_EDGE;

        (block_up, block_down, block_left, block_right)
    }



        // ---- FEN notation ----
    

    fn pos_to_str(pos: u8) -> String {
        let x = pos % 9;
        let y = pos / 9;
        let col = (b'a' + x as u8) as char;
        let row = (b'1' + y as u8) as char;
        format!("{}{}", col, row)
    }

    fn str_to_pos(s: &str) -> u8 {
        let bytes = s.as_bytes();
        let x = bytes[0] - b'a';
        let y = bytes[1] - b'1';
        y * 9 + x
    }

    fn wall_to_str(idx: u8, is_h: bool) -> String {
        let x = idx % 8;
        let y = idx / 8;
        let col = (b'a' + x) as char;
        let row = (b'1' + y) as char;
        let dir = if is_h { 'h' } else { 'v' };
        format!("{}{}{}", col, row, dir)
    }
}


// =================
// Python interface
// =================

#[pymethods]
impl GameState {
    
    #[new]
    pub fn new() -> Self {
        GameState::initial_state()
    }

    #[staticmethod]
    pub fn initial_state() -> Self {
        let z = get_zobrist();
        let p0_start = 4;
        let p1_start = 76;
        
        // initial hash only covers the starting positions
        let initial_hash = z.p0_pos[p0_start as usize] ^ z.p1_pos[p1_start as usize];

        GameState {
            positions: [p0_start, p1_start],
            walls_left: [10, 10],
            player_to_move: 0,
            walls_h: 0,
            walls_v: 0, 
            hash: initial_hash,
        }
    }

    #[getter]
    fn get_positions(&self) -> (u8, u8) { (self.positions[0], self.positions[1]) }

    #[getter]
    fn get_walls_left(&self) -> (u8, u8) { (self.walls_left[0], self.walls_left[1]) }

    #[getter]
    fn get_player_to_move(&self) -> u8 { self.player_to_move }

    fn __repr__(&self) -> String {
        format!("GameState(p0:{}, p1:{}, turn:{}, walls_h:{:016X}, walls_v:{:016X})",
            self.positions[0], self.positions[1], self.player_to_move, self.walls_h, self.walls_v)
    }




    pub fn is_game_finished(&self) -> i8 {
        if self.positions[0] >= 72 { return 0; }
        if self.positions[1] <= 8 { return 1; }
        -1
    }

    pub fn apply_move(&self, move_type: u8, x: u8, y: u8) -> Self {
        let mut next_state = *self;
        let p = self.player_to_move as usize;
        let z = get_zobrist();
 
        next_state.hash ^= z.player_turn; 

        match move_type {
            0 => {
                let old_pos = self.positions[p] as usize;
                let new_pos = GameState::xy_to_cell_idx(x, y) as usize;
                next_state.positions[p] = new_pos as u8;
                
                if p == 0 {
                    next_state.hash ^= z.p0_pos[old_pos] ^ z.p0_pos[new_pos];
                } else {
                    next_state.hash ^= z.p1_pos[old_pos] ^ z.p1_pos[new_pos];
                }
            }
            1 => {
                let idx = GameState::xy_to_wall_idx(x, y);
                next_state.walls_left[p] -= 1;
                next_state.walls_v |= 1u64 << idx;
                
                next_state.hash ^= z.walls_v[idx as usize];
            }
            2 => {
                let idx = GameState::xy_to_wall_idx(x, y);
                next_state.walls_left[p] -= 1;
                next_state.walls_h |= 1u64 << idx;
                
                next_state.hash ^= z.walls_h[idx as usize];
            }
            _ => unreachable!(),
        }

        next_state.player_to_move = 1 - self.player_to_move;
        next_state
    }

    pub fn has_path_to_goal(&self, player: u8) -> bool {
        let (b_up, b_down, b_left, b_right) = self.get_wall_blockers();
        
        let allow_up = !b_up;
        let allow_down = !b_down;
        let allow_left = !b_left;
        let allow_right = !b_right;

        let target_mask = if player == 0 { Self::BOTTOM_EDGE } else { Self::TOP_EDGE };
        let mut reach = 1u128 << self.positions[player as usize];

        loop {
            if (reach & target_mask) != 0 { return true; }

            let next_reach = reach 
                | ((reach & allow_up) >> 9)
                | ((reach & allow_down) << 9)
                | ((reach & allow_left) >> 1)
                | ((reach & allow_right) << 1);

            if next_reach == reach { return false; }
            reach = next_reach;
        }
    }

    /// BFS shortest-path distance plus structural metrics (reachable area, forced
    /// detour cost, tunnel count, immunity) used by the evaluation function.
    pub fn get_distances(&self) -> (i32, u32, f32, f32, bool, i32, u32, f32, f32, bool) { 
        let (b_up, b_down, b_left, b_right) = self.get_wall_blockers();
        
        let allow_up = !b_up;
        let allow_down = !b_down;
        let allow_left = !b_left;
        let allow_right = !b_right;

        let flood_fill = |player: u8, target_mask: u128| -> (i32, u32, f32, f32, bool) {
            let start_pos = self.positions[player as usize];
            let mut reach = 1u128 << start_pos;
            let mut distance = 0;
            let mut history = [0u128; 128]; // reach mask at each BFS step, for backtracking

            loop {
                history[distance as usize] = reach; 

                if (reach & target_mask) != 0 { 
                    let surf = reach.count_ones(); // total cells reachable from start
                    
                    // backtrack from the goal to the start to recover the shortest path
                    let mut path_mask = reach & target_mask;
                    
                    let mut max_delta = 0.0;
                    let mut tunnel_security = 0.0;
                    let mut immunity = false;

                    if distance > 0 {
                        for d in (0..distance).rev() {
                            let prev_expanded = path_mask 
                                | ((path_mask & allow_up) >> 9)
                                | ((path_mask & allow_down) << 9)
                                | ((path_mask & allow_left) >> 1)
                                | ((path_mask & allow_right) << 1);

                            // intersect with the previous wave to isolate the exact path cell(s)
                            path_mask = prev_expanded & history[d as usize];

                            // a corridor walled on both sides is a tunnel: no detour possible
                            let walled_left = (path_mask & allow_left) == 0;
                            let walled_right = (path_mask & allow_right) == 0;
                            let walled_up = (path_mask & allow_up) == 0;
                            let walled_down = (path_mask & allow_down) == 0;

                            if (walled_left && walled_right) || (walled_up && walled_down) {
                                tunnel_security += 1.0; 
                            }

                            // a 1-2 cell-wide path is a potential bottleneck the opponent
                            // could wall off; estimate the cost of the alternate route
                            if path_mask.count_ones() <= 2 && !immunity {
                                let blocked_mask = path_mask;
                                let mut alt_reach = 1u128 << start_pos;
                                let mut alt_dist = 0;
                                let mut found = false;

                                // BFS again, with the bottleneck cell(s) removed
                                for _ in 0..100 { 
                                    if (alt_reach & target_mask) != 0 {
                                        found = true;
                                        break;
                                    }
                                    let mut next_alt = alt_reach 
                                        | ((alt_reach & allow_up) >> 9)
                                        | ((alt_reach & allow_down) << 9)
                                        | ((alt_reach & allow_left) >> 1)
                                        | ((alt_reach & allow_right) << 1);

                                    next_alt &= !blocked_mask;

                                    if next_alt == alt_reach { break; } // no alternate route
                                    alt_reach = next_alt;
                                    alt_dist += 1;
                                }

                                if !found {
                                    // no alternate route exists, so walling the bottleneck would
                                    // strand the opponent — that wall placement is illegal
                                    immunity = true; 
                                } else {
                                    let delta = (alt_dist - distance) as f32;
                                    if delta > max_delta {
                                        max_delta = delta;
                                    }
                                }
                            }
                        }
                    }
                    
                    return (distance, surf, max_delta, tunnel_security, immunity); 
                }

                let next_reach = reach 
                    | ((reach & allow_up) >> 9)
                    | ((reach & allow_down) << 9)
                    | ((reach & allow_left) >> 1)
                    | ((reach & allow_right) << 1);

                if next_reach == reach { return (-1, 0, 0.0, 0.0, false); } 
                
                reach = next_reach;
                distance += 1;
            }
        };

        let (dist_0, surf_0, delta_0, tun_0, imm_0) = flood_fill(0, Self::BOTTOM_EDGE);
        let (dist_1, surf_1, delta_1, tun_1, imm_1) = flood_fill(1, Self::TOP_EDGE);

        (dist_0, surf_0, delta_0, tun_0, imm_0, dist_1, surf_1, delta_1, tun_1, imm_1)
    }


    pub fn is_state_valid(&self) -> bool {
        self.has_path_to_goal(0) && self.has_path_to_goal(1)
    }
    






    // ---- FEN notation ----


    /// Returns the FEN string for this board.
    pub fn get_FEN(&self) -> String {
        let p0 = Self::pos_to_str(self.positions[0]);
        let p1 = Self::pos_to_str(self.positions[1]);
        let w0 = self.walls_left[0];
        let w1 = self.walls_left[1];
        
        let mut walls = Vec::new();
        
        let mut h = self.walls_h;
        while h != 0 {
            let idx = h.trailing_zeros() as u8;
            walls.push(Self::wall_to_str(idx, true));
            h &= h - 1;
        }
        
        let mut v = self.walls_v;
        while v != 0 {
            let idx = v.trailing_zeros() as u8;
            walls.push(Self::wall_to_str(idx, false));
            v &= v - 1;
        }
        
        let walls_str = if walls.is_empty() { String::new() } else { walls.join(",") };
        let turn = if self.player_to_move == 0 { "R" } else { "B" };
        
        format!("{}/{}/{}/{}/{} {}", p0, p1, w0, w1, walls_str, turn)
    }

    /// Loads a board from a FEN string.
    #[staticmethod]
    pub fn load_from_FEN(fen: &str) -> pyo3::PyResult<Self> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(pyo3::exceptions::PyValueError::new_err("Invalid FEN: missing space separator"));
        }
        
        let board_parts: Vec<&str> = parts[0].split('/').collect();
        if board_parts.len() != 5 {
            return Err(pyo3::exceptions::PyValueError::new_err("Invalid FEN: expected 5 fields"));
        }
        
        let p0 = Self::str_to_pos(board_parts[0]);
        let p1 = Self::str_to_pos(board_parts[1]);
        let w0: u8 = board_parts[2].parse().unwrap_or(0);
        let w1: u8 = board_parts[3].parse().unwrap_or(0);
        
        let mut walls_h = 0u64;
        let mut walls_v = 0u64;
        
        if !board_parts[4].is_empty() {
            for w in board_parts[4].split(',') {
                let bytes = w.as_bytes();
                if bytes.len() >= 3 {
                    let x = bytes[0] - b'a';
                    let y = bytes[1] - b'1';
                    let idx = y * 8 + x;
                    if bytes[2] == b'h' {
                        walls_h |= 1u64 << idx;
                    } else if bytes[2] == b'v' {
                        walls_v |= 1u64 << idx;
                    }
                }
            }
        }
        
        let player_to_move = if parts[1] == "R" { 0 } else { 1 };
        
        // recompute the full Zobrist hash from scratch
        let z = get_zobrist();
        let mut hash = 0u64;
        
        hash ^= z.p0_pos[p0 as usize];
        hash ^= z.p1_pos[p1 as usize];
        
        let mut temp_h = walls_h;
        while temp_h != 0 {
            let idx = temp_h.trailing_zeros() as usize;
            hash ^= z.walls_h[idx];
            temp_h &= temp_h - 1;
        }
        
        let mut temp_v = walls_v;
        while temp_v != 0 {
            let idx = temp_v.trailing_zeros() as usize;
            hash ^= z.walls_v[idx];
            temp_v &= temp_v - 1;
        }
        
        if player_to_move == 1 {
            hash ^= z.player_turn;
        }
        
        Ok(GameState {
            positions: [p0, p1],
            walls_left: [w0, w1],
            player_to_move,
            walls_h,
            walls_v,
            hash,
        })
    }
}








#[pyclass]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    #[pyo3(get)]
    pub move_type: u8, // 0: pawn move, 1: vertical wall, 2: horizontal wall
    #[pyo3(get)]
    pub x: u8,
    #[pyo3(get)]
    pub y: u8,
}

#[pymethods]
impl Move {
    fn __repr__(&self) -> String {
        let m_type = match self.move_type {
            0 => "Move",
            1 => "Wall_V",
            2 => "Wall_H",
            _ => "Unknown",
        };
        format!("Move(type:{}, x:{}, y:{})", m_type, self.x, self.y)
    }
}







#[pyclass]
pub struct Engine {
    #[pyo3(get)]
    pub nodes_explored: u64,
    tt: std::collections::HashMap<u64, TTEntry>,

    // 2 killer moves per ply
    killer_moves: [[Option<Move>; 2]; 128],


    // time control
    start_time: Option<Instant>,
    time_limit_ms: u64,
    abort_search: bool,

    weights: EvalWeights,
}




impl Engine {
    // bit masks to prevent wraparound on the 8x8 wall grid
    // NOT_COL_A: all bits set except the leftmost column (x=0)
    const NOT_COL_A: u64 = 0xFEFEFEFEFEFEFEFE;
    // NOT_COL_H: all bits set except the rightmost column (x=7)
    const NOT_COL_H: u64 = 0x7F7F7F7F7F7F7F7F;





    /// --- Time control ---
    #[inline]
    fn check_time(&mut self) {
        if self.time_limit_ms > 0 {
            if let Some(start) = self.start_time {
                if start.elapsed().as_millis() as u64 >= self.time_limit_ms {
                    self.abort_search = true;
                }
            }
        }
    }







    /// --- Move generation ---

    /// Generates all legal walls (does not check for player entrapment).
    pub fn generate_legal_walls(state: &GameState, player: u8) -> Vec<Move> {
        let mut moves = Vec::with_capacity(128);

        if state.walls_left[player as usize] == 0 {
            return moves;
        }

        let w_h = state.walls_h;
        let w_v = state.walls_v;

        // a horizontal wall is blocked by: an existing H wall, an existing V wall
        // (crossing), or an H wall shifted one cell left or right
        let invalid_h = w_h 
                      | w_v 
                      | ((w_h << 1) & Self::NOT_COL_A) // mask prevents bits wrapping to the next row
                      | ((w_h >> 1) & Self::NOT_COL_H);
        
        let mut valid_h = !invalid_h;

        // Brian Kernighan's bit-clearing trick to iterate set bits
        while valid_h != 0 {
            let idx = valid_h.trailing_zeros() as u8;
            moves.push(Move { move_type: 2, x: idx % 8, y: idx / 8 });

            valid_h &= valid_h - 1; 
        }


        // a vertical wall is blocked by: an existing V wall, an existing H wall
        // (crossing), or a V wall shifted one cell up or down
        let invalid_v = w_v 
                      | w_h 
                      | (w_v << 8) 
                      | (w_v >> 8);
                      
        let mut valid_v = !invalid_v;

        while valid_v != 0 {
            let idx = valid_v.trailing_zeros() as u8;
            moves.push(Move { move_type: 1, x: idx % 8, y: idx / 8 });
            valid_v &= valid_v - 1;
        }

        moves
    }

    /// Generates all valid pawn moves, including jumps over the opponent.
    pub fn generate_pawn_moves(state: &GameState, player: u8) -> Vec<Move> {
        // a pawn has at most 5 possible moves (straight jump or diagonal jumps
        // around the opponent, plus the two side moves)
        let mut moves = Vec::with_capacity(5); 

        let pos = state.positions[player as usize];
        let opp = state.positions[(1 - player) as usize];
        
        let (b_up, b_down, b_left, b_right) = state.get_wall_blockers();
        
        let can_go = |p: u8, blockers: u128| -> bool {
            (blockers & (1u128 << p)) == 0
        };

        let mut add_move = |target_pos: u8| {
            moves.push(Move { move_type: 0, x: target_pos % 9, y: target_pos / 9 });
        };



        // --- UP ---
        if can_go(pos, b_up) {
            let next_pos = pos - 9;
            if next_pos != opp {
                add_move(next_pos);
            } else {
                // opponent is right there, try jumping over it
                if can_go(opp, b_up) {
                    add_move(opp - 9); // straight jump
                } else {
                    // straight jump blocked (wall or board edge) -> diagonal jumps
                    if can_go(opp, b_left) { add_move(opp - 1); }
                    if can_go(opp, b_right) { add_move(opp + 1); }
                }
            }
        }

        // --- DOWN ---
        if can_go(pos, b_down) {
            let next_pos = pos + 9;
            if next_pos != opp {
                add_move(next_pos);
            } else {
                if can_go(opp, b_down) {
                    add_move(opp + 9);
                } else {
                    if can_go(opp, b_left) { add_move(opp - 1); }
                    if can_go(opp, b_right) { add_move(opp + 1); }
                }
            }
        }

        // --- LEFT ---
        if can_go(pos, b_left) {
            let next_pos = pos - 1;
            if next_pos != opp {
                add_move(next_pos);
            } else {
                if can_go(opp, b_left) {
                    add_move(opp - 1);
                } else {
                    if can_go(opp, b_up) { add_move(opp - 9); }
                    if can_go(opp, b_down) { add_move(opp + 9); }
                }
            }
        }

        // --- RIGHT ---
        if can_go(pos, b_right) {
            let next_pos = pos + 1;
            if next_pos != opp {
                add_move(next_pos);
            } else {
                if can_go(opp, b_right) {
                    add_move(opp + 1);
                } else {
                    if can_go(opp, b_up) { add_move(opp - 9); }
                    if can_go(opp, b_down) { add_move(opp + 9); }
                }
            }
        }

        moves
    }




    // --- Heuristics ---
    // A positive score favors Player 0.



    pub fn evaluation_midgame(&self, state: &GameState, d0: f32, d1: f32, w0: f32, w1: f32, surf0: f32, surf1: f32, delta0: f32, delta1: f32, tunnel0: f32, tunnel1: f32, imm0: bool, imm1: bool) -> f32 {
        let w = &self.weights;
        let mut score = 0.0;

        // race to the goal
        score += (d1 - d0) * w.mg_course_mult;
        
        // reachable space
        score += (surf0 - surf1) * w.mg_surface_mult; 

        // bottleneck threat: penalty only matters if the opponent has walls left to exploit it
        score -= delta0 * (w1 * w.mg_delta_penalty_mult);
        score += delta1 * (w0 * w.mg_delta_penalty_mult);

        // tunnel safety
        score += tunnel0 * w.mg_tunnel_bonus;
        score -= tunnel1 * w.mg_tunnel_bonus;

        // immunity: path can't legally be walled off
        if imm0 { score += w.mg_immunity_bonus; }
        if imm1 { score -= w.mg_immunity_bonus; }

        // wall count
        let total_dist = d0 + d1;
        let wall_value = w.mg_wall_base_value + w.mg_wall_inflation / (total_dist + 2.0); 
        score += (w0 - w1) * wall_value;

        if w0 > w1 { score += w.mg_parity_bonus; } 
        else if w1 > w0 { score -= w.mg_parity_bonus; }

        if w0 <= 2.0 && w1 > w0 { score -= (w1 - w0) * w.mg_scarcity_penalty; }
        if w1 <= 2.0 && w0 > w1 { score += (w0 - w1) * w.mg_scarcity_penalty; }

        if w0 == 0.0 && w1 > 0.0 { score -= w.mg_panic_penalty; }
        if w1 == 0.0 && w0 > 0.0 { score += w.mg_panic_penalty; }

        // tempo and gravity
        if state.player_to_move == 0 { score += w.mg_tempo_bonus; } 
        else { score -= w.mg_tempo_bonus; }

        let y0 = (state.positions[0] / 9) as f32;               
        let y1 = 8.0 - (state.positions[1] / 9) as f32;         
        score += (y0 - y1) * w.mg_gravity_bonus; 

        score
    }

    pub fn evaluation_endgame(&self, state: &GameState, d0: f32, d1: f32, w0: f32, w1: f32, surf0: f32, surf1: f32, delta0: f32, delta1: f32, tunnel0: f32, tunnel1: f32, imm0: bool, imm1: bool) -> f32 {
        let w = &self.weights;
        let mut score = 0.0;

        // same terms as evaluation_midgame, with the endgame weight set
        score += (d1 - d0) * w.eg_course_mult;
        score += (surf0 - surf1) * w.eg_surface_mult; 

        score -= delta0 * (w1 * w.eg_delta_penalty_mult);
        score += delta1 * (w0 * w.eg_delta_penalty_mult);

        score += tunnel0 * w.eg_tunnel_bonus;
        score -= tunnel1 * w.eg_tunnel_bonus;

        if imm0 { score += w.eg_immunity_bonus; }
        if imm1 { score -= w.eg_immunity_bonus; }

        let total_dist = d0 + d1;
        let wall_value = w.eg_wall_base_value + w.eg_wall_inflation / (total_dist + 2.0); 
        score += (w0 - w1) * wall_value;

        if w0 > w1 { score += w.eg_parity_bonus; } 
        else if w1 > w0 { score -= w.eg_parity_bonus; }

        if w0 <= 2.0 && w1 > w0 { score -= (w1 - w0) * w.eg_scarcity_penalty; }
        if w1 <= 2.0 && w0 > w1 { score += (w0 - w1) * w.eg_scarcity_penalty; }

        if w0 == 0.0 && w1 > 0.0 { score -= w.eg_panic_penalty; }
        if w1 == 0.0 && w0 > 0.0 { score += w.eg_panic_penalty; }

        if state.player_to_move == 0 { score += w.eg_tempo_bonus; } 
        else { score -= w.eg_tempo_bonus; }

        let y0 = (state.positions[0] / 9) as f32;               
        let y1 = 8.0 - (state.positions[1] / 9) as f32;         
        score += (y0 - y1) * w.eg_gravity_bonus; 

        score
    }



    pub fn evaluate(&self, state: &GameState) -> f32 {
        let (d0_i, surf0_i, delta0, tunnel0, imm0, d1_i, surf1_i, delta1, tunnel1, imm1) = state.get_distances();

        if d0_i == -1 { return -10000.0; }
        if d1_i == -1 { return 10000.0; }

        let d0 = d0_i as f32; let d1 = d1_i as f32;
        let w0 = state.walls_left[0] as f32; let w1 = state.walls_left[1] as f32;
        let surf0 = surf0_i as f32; let surf1 = surf1_i as f32;

        let d_score = self.evaluation_midgame(state, d0, d1, w0, w1, surf0, surf1, delta0, delta1, tunnel0, tunnel1, imm0, imm1);
        let f_score = self.evaluation_endgame(state, d0, d1, w0, w1, surf0, surf1, delta0, delta1, tunnel0, tunnel1, imm0, imm1);


        // blend midgame/endgame scores based on total walls remaining
        let x = (w0 + w1) / 20.0;
        let a = 0.15; 
        let b = 0.7;

        let mut factor;


        if x <= a {
            factor = 0.0 // few walls left: pure endgame score
        } 
        else if x >= b {
            factor = 1.0 // many walls left: pure midgame score
        }
        else {
            let u = (x - a) / (b - a);
            factor = 3.0 * u * u - 2.0 * u * u * u // smoothstep transition
        };



        let mut final_score = f_score + factor * (d_score - f_score);

        // small terms outside the blend: urgency near the goal, plus tie-breaking noise
        final_score += 30.0 / (d0 + 1.0) - 30.0 / (d1 + 1.0);
        final_score += (state.hash % 100) as f32 * 0.001; 

        final_score
    }


    /// Sorts moves in place to maximize alpha-beta cutoffs.
    pub fn sort_moves(state: &GameState, moves: &mut [Move], killers: &[Option<Move>; 2]) {
        let opp = 1 - state.player_to_move;
        let opp_pos = state.positions[opp as usize];
        
        let opp_x = (opp_pos % 9) as i32;
        let opp_y = (opp_pos / 9) as i32;

        moves.sort_unstable_by_key(|m| {
            let mut score = if m.move_type == 0 {
                10000  // pawn moves first
            } else {
                let dx = (m.x as i32) - opp_x;
                let dy = (m.y as i32) - opp_y;
                -(dx * dx + dy * dy) // walls closer to the opponent score higher
            };
            
            // killer move bonus: tested right after the TT move
            if Some(*m) == killers[0] {
                score += 20000;
            } else if Some(*m) == killers[1] {
                score += 15000;
            }
            
            // Reverse to sort by descending score
            std::cmp::Reverse(score)
        });
    }



    /// ---- Minimax ----


    fn minimax(&mut self, state: GameState, depth: u8, ply: usize, mut alpha: f32, mut beta: f32, is_maximizer: bool) -> (f32, Option<Move>) {
        self.nodes_explored += 1;
        let orig_alpha = alpha;
        let orig_beta = beta;


        // time control
        if (self.nodes_explored & 2047) == 0 {
            self.check_time();
        }
        if self.abort_search {
            return (0.0, None); 
        }


        // clamp to avoid overflowing the killer_moves array
        let safe_ply = ply.min(127);


        let mut tt_move: Option<Move> = None;
        if let Some(entry) = self.tt.get(&state.hash) {
            tt_move = entry.best_move;
            
            if entry.depth >= depth {
                match entry.flag {
                    FLAG_EXACT => return (entry.score, entry.best_move),
                    FLAG_LOWERBOUND => alpha = alpha.max(entry.score),
                    FLAG_UPPERBOUND => beta = beta.min(entry.score),
                    _ => {}
                }
                if alpha >= beta {
                    return (entry.score, entry.best_move);
                }
            }
        }


        // terminal conditions
        let finished = state.is_game_finished();
        if finished == 0 { return (10000.0 + depth as f32, None); }
        if finished == 1 { return (-10000.0 - depth as f32, None); }
        

        if depth == 0 { 
            let q_score = self.quiescence(state, alpha, beta, is_maximizer, ply, 0);
            return (q_score, None); 
        }



        // move generation and ordering
        let killers = self.killer_moves[safe_ply];
        let mut moves = Self::get_all_moves(&state, state.player_to_move);
        

        // if the TT suggests a move, try it first
        if let Some(tm) = tt_move {
            if let Some(pos) = moves.iter().position(|&m| m == tm) {
                moves.swap(0, pos);
            }
        } else {
            Self::sort_moves(&state, &mut moves, &killers);
        }


        

        let mut best_move = None;
        let mut best_score = if is_maximizer { f32::NEG_INFINITY } else { f32::INFINITY };

        for (i, m) in moves.iter().enumerate() {
            let child_state = state.apply_move(m.move_type, m.x, m.y);

            if m.move_type != 0 {
                if !child_state.is_state_valid() { continue; }
            }

            let mut value: f32;
            let is_late_move = i >= 4;
            let is_wall = m.move_type != 0;

            // late move reduction: search late wall moves at reduced depth first,
            // and only re-search at full depth if they look like they might improve alpha/beta
            if depth >= 3 && is_late_move && is_wall {
                value = self.minimax(child_state, depth - 2, ply + 1, alpha, beta, !is_maximizer).0;
                if is_maximizer && value > alpha {
                    value = self.minimax(child_state, depth - 1, ply + 1, alpha, beta, !is_maximizer).0;
                } else if !is_maximizer && value < beta {
                    value = self.minimax(child_state, depth - 1, ply + 1, alpha, beta, !is_maximizer).0;
                }
            } else {
                value = self.minimax(child_state, depth - 1, ply + 1, alpha, beta, !is_maximizer).0;
            }



            if is_maximizer {
                if value > best_score {
                    best_score = value;
                    best_move = Some(*m);
                }
                alpha = alpha.max(best_score);
                if best_score >= beta { 
                    // beta cutoff: record as a killer move for this ply
                    if self.killer_moves[safe_ply][0] != Some(*m) {
                        self.killer_moves[safe_ply][1] = self.killer_moves[safe_ply][0];
                        self.killer_moves[safe_ply][0] = Some(*m);
                    }
                    break; 
                }
            } else {
                if value < best_score {
                    best_score = value;
                    best_move = Some(*m);
                }
                beta = beta.min(best_score);
                if best_score <= alpha { 
                    // alpha cutoff: record as a killer move for this ply
                    if self.killer_moves[safe_ply][0] != Some(*m) {
                        self.killer_moves[safe_ply][1] = self.killer_moves[safe_ply][0];
                        self.killer_moves[safe_ply][0] = Some(*m);
                    }
                    break; 
                }
            }
        }



        // store result in the transposition table
        let flag = if best_score <= orig_alpha {
            FLAG_UPPERBOUND
        } else if best_score >= orig_beta {
            FLAG_LOWERBOUND
        } else {
            FLAG_EXACT
        };

        self.tt.insert(state.hash, TTEntry {
            depth,
            score: best_score,
            flag,
            best_move,
        });

        (best_score, best_move)
    }



    /// Quiescence search: extends the search past violent moves to avoid the
    /// horizon effect.
    ///
    /// NOTE: the early return below makes everything after it unreachable.
    fn quiescence(&mut self, state: GameState, mut alpha: f32, mut beta: f32, is_maximizer: bool, ply: usize, q_depth: u8) -> f32 {
        self.nodes_explored += 1;


        // time control
        if (self.nodes_explored & 2047) == 0 {
            self.check_time();
        }
        if self.abort_search {
            return 0.0;
        }


        let finished = state.is_game_finished();
        if finished == 0 { return 10000.0; }
        if finished == 1 { return -10000.0; }
        
        if q_depth >= 2 { return self.evaluate(&state); } // quiescence depth cutoff

        // stand-pat: assume "doing nothing" is always an option
        let stand_pat = self.evaluate(&state);

        if is_maximizer {
            if stand_pat >= beta { return beta; }
            alpha = alpha.max(stand_pat);
        } else {
            if stand_pat <= alpha { return alpha; }
            beta = beta.min(stand_pat);
        }


        let mut moves = Self::generate_pawn_moves(&state, state.player_to_move);
        let safe_ply = ply.min(127);
        
        if state.walls_left[state.player_to_move as usize] > 0 {
            let legal_walls = Self::generate_legal_walls(&state, state.player_to_move);
            
            if let Some(k1) = self.killer_moves[safe_ply][0] {
                if k1.move_type != 0 && legal_walls.contains(&k1) { moves.push(k1); } // killer move must still be a legal wall
            }
            if let Some(k2) = self.killer_moves[safe_ply][1] {
                if k2.move_type != 0 && legal_walls.contains(&k2) && Some(k2) != self.killer_moves[safe_ply][0] { 
                    moves.push(k2); 
                }
            }
        }

        let mut best_score = stand_pat;

        for m in moves {
            let child_state = state.apply_move(m.move_type, m.x, m.y);

            if m.move_type != 0 {
                if !child_state.is_state_valid() { continue; }
            }

            let value = self.quiescence(child_state, alpha, beta, !is_maximizer, ply + 1, q_depth + 1);

            if is_maximizer {
                best_score = best_score.max(value);
                alpha = alpha.max(best_score);
                if best_score >= beta { break; }
            } else {
                best_score = best_score.min(value);
                beta = beta.min(best_score);
                if best_score <= alpha { break; }
            }
        }

        best_score
    }
}





#[pymethods]
impl Engine {


    #[staticmethod]
    pub fn get_all_moves(state: &GameState, player: u8) -> Vec<Move> {
        let mut moves = Self::generate_pawn_moves(state, player);
        moves.extend(Self::generate_legal_walls(state, player));
        moves
    }
    
    #[new]
    pub fn new() -> Self {
        Engine {
            nodes_explored: 0,
            tt: std::collections::HashMap::with_capacity(8_000_000), 
            killer_moves: [[None; 2]; 128],
            start_time: None,
            time_limit_ms: 0,
            abort_search: false,
            weights: EvalWeights::new(),
        }
    }

    pub fn set_weights(&mut self, new_weights: EvalWeights) {
        self.weights = new_weights;
    }

    /// Clears the transposition table and killer moves (call between games).
    pub fn clear_cache(&mut self) {
        self.tt.clear();
        self.killer_moves = [[None; 2]; 128];
    }

    /// Entry point: searches for the best move under the given depth/time budget.
    pub fn get_best_move(&mut self, state: &GameState, max_depth: u8, time_limit_ms: u64, player_to_maximise: u8) -> Option<Move> {
        self.nodes_explored = 0;
        
        self.start_time = Some(Instant::now());
        self.time_limit_ms = time_limit_ms;
        self.abort_search = false;

        let mut overall_best_action = None;

        // iterative deepening
        for d in 1..=max_depth {
            self.killer_moves = [[None; 2]; 128]; 
            
            let (_, m) = self.minimax(*state, d, 0, f32::NEG_INFINITY, f32::INFINITY, state.player_to_move == player_to_maximise);
            

            if self.abort_search { // discard this depth's result if the search was cut short
                break;
            }
            
            if m.is_some() {
                overall_best_action = m;
            }
        }

        overall_best_action
    }
} 





const FLAG_EXACT: u8 = 0;
const FLAG_LOWERBOUND: u8 = 1;
const FLAG_UPPERBOUND: u8 = 2;

/// Transposition table entry.
#[derive(Clone, Copy)]
pub struct TTEntry {
    pub depth: u8,
    pub score: f32,
    pub flag: u8,
    pub best_move: Option<Move>,
}



// Python module init
#[pymodule]
fn kuyper(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<GameState>()?;
    m.add_class::<Move>()?; 
    m.add_class::<Engine>()?;
    m.add_class::<EvalWeights>()?; 
    Ok(())
}