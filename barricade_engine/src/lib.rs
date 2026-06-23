use pyo3::prelude::*;
use std::sync::OnceLock;
use std::time::Instant;






#[pyclass]
#[derive(Clone, Debug)]
pub struct EvalWeights {
    // --- COEFFS DE DÉBUT / MILIEU DE PARTIE (Middlegame) ---
    #[pyo3(get, set)] pub mg_course_mult: f32,
    #[pyo3(get, set)] pub mg_delta_penalty_mult: f32, // REMPLACE vuln_threat
    #[pyo3(get, set)] pub mg_surface_mult: f32,
    #[pyo3(get, set)] pub mg_tunnel_bonus: f32,       // REMPLACE clausto_threshold
    #[pyo3(get, set)] pub mg_immunity_bonus: f32,     // REMPLACE clausto_penalty
    #[pyo3(get, set)] pub mg_wall_base_value: f32,
    #[pyo3(get, set)] pub mg_wall_inflation: f32,
    #[pyo3(get, set)] pub mg_parity_bonus: f32,
    #[pyo3(get, set)] pub mg_scarcity_penalty: f32,
    #[pyo3(get, set)] pub mg_panic_penalty: f32,
    #[pyo3(get, set)] pub mg_tempo_bonus: f32,
    #[pyo3(get, set)] pub mg_gravity_bonus: f32,

    // --- COEFFS DE FIN DE PARTIE (Endgame) ---
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
            // --- COEFFS DE DÉBUT / MILIEU DE PARTIE (Middlegame) ---
            mg_course_mult: 6.46,
            mg_delta_penalty_mult: 1.10,
            mg_surface_mult: 0.50,      // L'IA a abandonné l'idée de surface (Bloqué au minimum)
            mg_tunnel_bonus: 1.90,
            mg_immunity_bonus: 19.47,   // Poids statique rare
            mg_wall_base_value: 3.87,
            mg_wall_inflation: 30.32,
            mg_parity_bonus: 2.26,
            mg_scarcity_penalty: 2.82,
            mg_panic_penalty: 15.08,
            mg_tempo_bonus: 1.75,
            mg_gravity_bonus: 1.00,

            // --- COEFFS DE FIN DE PARTIE (Endgame) ---
            eg_course_mult: 10.10,      // Sprint absolu sur-prioritaire
            eg_delta_penalty_mult: 2.49,
            eg_surface_mult: 0.00,      // Surface totalement ignorée
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







// Table de hashage 
pub struct ZobristTable {
    pub p0_pos: [u64; 81],
    pub p1_pos: [u64; 81],
    pub walls_h: [u64; 64],
    pub walls_v: [u64; 64],
    pub player_turn: u64, // Appliqué uniquement si c'est au tour de P1
}


impl ZobristTable {
    fn init() -> Self {
        let mut prng = 1070372_u64; // Seed
        
        // Petite fonction XorShift pour générer des nombres très aléatoires
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
//  MÉCANIQUE INTERNE PURE (Invisible et inaccessible par Python)
// =================================================================
impl GameState {
    const TOP_EDGE: u128 = 0x1FF;
    const BOTTOM_EDGE: u128 = 0x1FF << 72;
    const LEFT_EDGE: u128 = 1 | (1<<9) | (1<<18) | (1<<27) | (1<<36) | (1<<45) | (1<<54) | (1<<63) | (1<<72);
    const RIGHT_EDGE: u128 = Self::LEFT_EDGE << 8;

    #[inline]
    pub fn xy_to_cell_idx(x: u8, y: u8) -> u8 {
        debug_assert!(x < 9 && y < 9, "Erreur fatale: Case ({}, {}) hors du plateau", x, y);
        y * 9 + x
    }

    #[inline]
    pub fn xy_to_wall_idx(x: u8, y: u8) -> u8 {
        debug_assert!(x < 8 && y < 8, "Erreur fatale: Mur ({}, {}) hors grille", x, y);
        y * 8 + x
    }

    #[inline]
    fn expand_8x8_to_9x9(walls: u64) -> u128 {
        let mut res = 0u128;
        res |= walls as u128 & 0xFF; // Correction du Warning (parenthèses retirées)
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



        // ---- FEN Notation ----
    

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
// INTERFACE PYTHON 
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
        
        // Le Hash de base ne contient que les positions initiales 
        let initial_hash = z.p0_pos[p0_start as usize] ^ z.p1_pos[p1_start as usize];

        GameState {
            positions: [p0_start, p1_start],
            walls_left: [10, 10],
            player_to_move: 0,
            walls_h: 0,
            walls_v: 0, 
            hash: initial_hash, // On enregistre le hash
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
            let mut history = [0u128; 128]; // Mémorise la vague de propagation

            loop {
                history[distance as usize] = reach; 

                // --- OBJECTIF ATTEINT ---
                if (reach & target_mask) != 0 { 
                    let surf = reach.count_ones(); // La surface totale disponible
                    
                    // On isole la case d'arrivée pour retracer le chemin inverse
                    let mut path_mask = reach & target_mask;
                    
                    let mut max_delta = 0.0;
                    let mut tunnel_security = 0.0;
                    let mut immunity = false;

                    // --- RETRAÇAGE DU CHEMIN ET ANALYSE STRUCTURELLE ---
                    if distance > 0 {
                        // On remonte le chemin de l'arrivée jusqu'au départ
                        for d in (0..distance).rev() {
                            let prev_expanded = path_mask 
                                | ((path_mask & allow_up) >> 9)
                                | ((path_mask & allow_down) << 9)
                                | ((path_mask & allow_left) >> 1)
                                | ((path_mask & allow_right) << 1);

                            // Intersection avec la vague précédente pour trouver la case exacte du chemin
                            path_mask = prev_expanded & history[d as usize];

                            // 1. DÉTECTION DU CHEMIN FORCÉ (Le Tunnel)
                            let walled_left = (path_mask & allow_left) == 0;
                            let walled_right = (path_mask & allow_right) == 0;
                            let walled_up = (path_mask & allow_up) == 0;
                            let walled_down = (path_mask & allow_down) == 0;

                            // Si on a des murs parallèles des deux côtés, c'est un tunnel sécurisé
                            if (walled_left && walled_right) || (walled_up && walled_down) {
                                tunnel_security += 1.0; 
                            }

                            // 2. DÉTECTION DU DELTA ET DE L'IMMUNITÉ
                            // Si la largeur du chemin est de 1 ou 2 cases, c'est un goulot potentiellement dangereux
                            if path_mask.count_ones() <= 2 && !immunity {
                                let blocked_mask = path_mask;
                                let mut alt_reach = 1u128 << start_pos;
                                let mut alt_dist = 0;
                                let mut found = false;

                                // Mini BFS de secours ultra-rapide (On teste un chemin alternatif)
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

                                    // L'ASTUCE MAGIQUE : On bloque virtuellement le goulot !
                                    next_alt &= !blocked_mask;

                                    if next_alt == alt_reach { break; } // Impasse totale
                                    alt_reach = next_alt;
                                    alt_dist += 1;
                                }

                                if !found {
                                    // CHANTAGE À LA LÉGALITÉ : Ce chemin est incassable !
                                    // L'adversaire n'a pas le droit de poser un mur ici, on est immunisé.
                                    immunity = true; 
                                } else {
                                    // Calcul du vrai détour
                                    let delta = (alt_dist - distance) as f32;
                                    if delta > max_delta {
                                        max_delta = delta;
                                    }
                                }
                            }
                        }
                    }
                    
                    // On retourne toutes les métriques
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

        // On lance le super-scanner pour le Joueur 0 (Rouge) et le Joueur 1 (Bleu)
        let (dist_0, surf_0, delta_0, tun_0, imm_0) = flood_fill(0, Self::BOTTOM_EDGE);
        let (dist_1, surf_1, delta_1, tun_1, imm_1) = flood_fill(1, Self::TOP_EDGE);

        (dist_0, surf_0, delta_0, tun_0, imm_0, dist_1, surf_1, delta_1, tun_1, imm_1)
    }


    pub fn is_state_valid(&self) -> bool {
        self.has_path_to_goal(0) && self.has_path_to_goal(1)
    }
    






    // ---- FEN Notation ----
    

    /// Renvoie la chaine FEN d'un plateau
    pub fn get_FEN(&self) -> String {
        let p0 = Self::pos_to_str(self.positions[0]);
        let p1 = Self::pos_to_str(self.positions[1]);
        let w0 = self.walls_left[0];
        let w1 = self.walls_left[1];
        
        let mut walls = Vec::new();
        
        // On extrait les murs horizontaux
        let mut h = self.walls_h;
        while h != 0 {
            let idx = h.trailing_zeros() as u8;
            walls.push(Self::wall_to_str(idx, true));
            h &= h - 1;
        }
        
        // On extrait les murs verticaux
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

    /// Charge un plateau à partir d'une chaîne FEN
    #[staticmethod]
    pub fn load_from_FEN(fen: &str) -> pyo3::PyResult<Self> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(pyo3::exceptions::PyValueError::new_err("Format FEN invalide (Espace manquant)"));
        }
        
        let board_parts: Vec<&str> = parts[0].split('/').collect();
        if board_parts.len() != 5 {
            return Err(pyo3::exceptions::PyValueError::new_err("Format FEN invalide (5 blocs requis)"));
        }
        
        let p0 = Self::str_to_pos(board_parts[0]);
        let p1 = Self::str_to_pos(board_parts[1]);
        let w0: u8 = board_parts[2].parse().unwrap_or(0);
        let w1: u8 = board_parts[3].parse().unwrap_or(0);
        
        let mut walls_h = 0u64;
        let mut walls_v = 0u64;
        
        // Parsing des murs
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
        
        // Calcul complet de la table Zobrist
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
    pub move_type: u8, // 0: Déplacement, 1: Mur Vertical, 2: Mur Horizontal
    #[pyo3(get)]
    pub x: u8,
    #[pyo3(get)]
    pub y: u8,
}

#[pymethods]
impl Move {
    fn __repr__(&self) -> String {
        let m_type = match self.move_type {
            0 => "Déplacement",
            1 => "Mur_V",
            2 => "Mur_H",
            _ => "Inconnu",
        };
        format!("Move(type:{}, x:{}, y:{})", m_type, self.x, self.y)
    }
}







#[pyclass]
pub struct Engine {
    #[pyo3(get)]
    pub nodes_explored: u64,
    // HashMap natif ultra-rapide de Rust
    tt: std::collections::HashMap<u64, TTEntry>,

    // Mémoire des Killer Moves (2 coups par niveau de profondeur)
    killer_moves: [[Option<Move>; 2]; 128],


    // restriction de temps 
    start_time: Option<Instant>,
    time_limit_ms: u64,
    abort_search: bool,

    // Poids de l'évaluation 
    weights: EvalWeights,
}




impl Engine {
    // Masques pour éviter les débordements de ligne sur la grille 8x8
    // NOT_COL_A : Tous les bits à 1 SAUF la colonne de gauche (x=0)
    const NOT_COL_A: u64 = 0xFEFEFEFEFEFEFEFE;
    // NOT_COL_H : Tous les bits à 1 SAUF la colonne de droite (x=7)
    const NOT_COL_H: u64 = 0x7F7F7F7F7F7F7F7F;





    /// --- Gestion du temps ---
    #[inline]
    fn check_time(&mut self) {
        if self.time_limit_ms > 0 {
            if let Some(start) = self.start_time {
                if start.elapsed().as_millis() as u64 >= self.time_limit_ms {
                    self.abort_search = true; // On déclenche l'alarme !
                }
            }
        }
    }







    /// --- Générations des coups ---

    /// Génère tous les murs légaux (sans vérifier l'enfermement).
    pub fn generate_legal_walls(state: &GameState, player: u8) -> Vec<Move> {
        let mut moves = Vec::with_capacity(128); // Pré-allocation 

        // Si le joueur n'a plus de murs, on s'arrête tout de suite.
        if state.walls_left[player as usize] == 0 {
            return moves;
        }

        let w_h = state.walls_h;
        let w_v = state.walls_v;

        // MURS HORIZONTAUX
        // Un mur H est bloqué par : un mur H existant, un mur V existant (croisement), 
        // un mur H décalé à gauche, ou un mur H décalé à droite.
        let invalid_h = w_h 
                      | w_v 
                      | ((w_h << 1) & Self::NOT_COL_A) // NOT_COL_A : évite le saut de bits de gauche a droite 
                      | ((w_h >> 1) & Self::NOT_COL_H);
        
        let mut valid_h = !invalid_h;


        // Algorithme de Brian Kernighan
        while valid_h != 0 {
            // .trailing_zeros() compte les zéros à droite
            let idx = valid_h.trailing_zeros() as u8; // directement implémenté dans le CPU
            moves.push(Move { move_type: 2, x: idx % 8, y: idx / 8 });

            valid_h &= valid_h - 1; 
        }


        // MURS VERTICAUX
        // Un mur V est bloqué par : un mur V existant, un mur H existant (croisement),
        // un mur V décalé vers le haut (<< 8), ou un mur V décalé vers le bas (>> 8).
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

    /// Génère tous les mouvements de pions valides, y compris les sauts complexes.
    pub fn generate_pawn_moves(state: &GameState, player: u8) -> Vec<Move> {
        // Un pion a au maximum 5 coups possibles (ex: bloqué devant, saute en diagonale gauche/droite, + arrière + côtés)
        let mut moves = Vec::with_capacity(5); 

        let pos = state.positions[player as usize];
        let opp = state.positions[(1 - player) as usize];
        
        // Limites du plateau
        let (b_up, b_down, b_left, b_right) = state.get_wall_blockers();
        
        // Fonction interne
        let can_go = |p: u8, blockers: u128| -> bool {
            (blockers & (1u128 << p)) == 0
        };

        // Fonction interne -- formate l'ajout d'un coup
        let mut add_move = |target_pos: u8| {
            moves.push(Move { move_type: 0, x: target_pos % 9, y: target_pos / 9 });
        };



        // --- HAUT ---
        if can_go(pos, b_up) {
            let next_pos = pos - 9;
            if next_pos != opp {
                add_move(next_pos); // Déplacement normal
            } else {
                // L'adversaire est devant. On tente d'aller une case plus loins
                if can_go(opp, b_up) {
                    add_move(opp - 9); // Saut Droit
                } else {
                    // Saut droit bloqué (par un mur ou le bord du plateau) -> Sauts Diagonaux
                    if can_go(opp, b_left) { add_move(opp - 1); }
                    if can_go(opp, b_right) { add_move(opp + 1); }
                }
            }
        }

        // --- BAS ---
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

        // --- GAUCHE ---
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

        // --- DROITE ---
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




    // --- Heuristiques ---
    // Un score positif -> avantage pour le Joueur 0.
    // Un score positif -> avantage pour le Joueur 0.



    pub fn evaluation_midgame(&self, state: &GameState, d0: f32, d1: f32, w0: f32, w1: f32, surf0: f32, surf1: f32, delta0: f32, delta1: f32, tunnel0: f32, tunnel1: f32, imm0: bool, imm1: bool) -> f32 {
        let w = &self.weights;
        let mut score = 0.0;

        // 1. La Course brute
        score += (d1 - d0) * w.mg_course_mult;
        
        // 2. La Surface (Espace disponible)
        score += (surf0 - surf1) * w.mg_surface_mult; 

        // 3. NOUVEAU : La Menace Structurelle (Le prix du vrai détour)
        // La pénalité n'est appliquée que si l'adversaire a des murs pour exploiter le goulot !
        score -= delta0 * (w1 * w.mg_delta_penalty_mult);
        score += delta1 * (w0 * w.mg_delta_penalty_mult);

        // 4. NOUVEAU : La Sécurité des Tunnels
        score += tunnel0 * w.mg_tunnel_bonus;
        score -= tunnel1 * w.mg_tunnel_bonus;

        // 5. NOUVEAU : L'Immunité (Le chantage à la légalité)
        if imm0 { score += w.mg_immunity_bonus; }
        if imm1 { score -= w.mg_immunity_bonus; }

        // 6. Gestion Matérielle
        let total_dist = d0 + d1;
        let wall_value = w.mg_wall_base_value + w.mg_wall_inflation / (total_dist + 2.0); 
        score += (w0 - w1) * wall_value;

        if w0 > w1 { score += w.mg_parity_bonus; } 
        else if w1 > w0 { score -= w.mg_parity_bonus; }

        if w0 <= 2.0 && w1 > w0 { score -= (w1 - w0) * w.mg_scarcity_penalty; }
        if w1 <= 2.0 && w0 > w1 { score += (w0 - w1) * w.mg_scarcity_penalty; }

        if w0 == 0.0 && w1 > 0.0 { score -= w.mg_panic_penalty; }
        if w1 == 0.0 && w0 > 0.0 { score += w.mg_panic_penalty; }

        // 7. Temps et Gravité
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

        // Même logique mais avec les poids de fin de partie
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


        let x = (w0 + w1) / 20.0;
        let a = 0.15; 
        let b = 0.7;

        let mut factor;


        if x <= a {
            factor = 0.0 // Zone morte : 100% Endgame score
        } 
        if x >= b {
            factor = 1.0 // Zone morte : 100% Midgame score
        }
        else {
            let u = (x - a) / (b - a);
            factor = 3.0 * u * u - 2.0 * u * u * u // Courbe de transition smoothstep
        };



        // Interpolation finale
        let mut final_score = f_score + factor * (d_score - f_score);

        // Intégration des termes absolus invariables (Urgence + Bruit)
        final_score += 30.0 / (d0 + 1.0) - 30.0 / (d1 + 1.0);
        final_score += (state.hash % 100) as f32 * 0.001; 

        final_score
    }


    /// Trie les coups sur place pour maximiser les coupures Alpha-Beta.
    pub fn sort_moves(state: &GameState, moves: &mut [Move], killers: &[Option<Move>; 2]) {
        let opp = 1 - state.player_to_move;
        let opp_pos = state.positions[opp as usize];
        
        // Coordonnées de l'adversaire
        let opp_x = (opp_pos % 9) as i32;
        let opp_y = (opp_pos / 9) as i32;

        // Tri ultra-rapide sur place (in-place)
        moves.sort_unstable_by_key(|m| {
            let mut score = if m.move_type == 0 {
                10000  // Pions
            } else {
                let dx = (m.x as i32) - opp_x;
                let dy = (m.y as i32) - opp_y;
                -(dx * dx + dy * dy) // Murs proches
            };
            
            // NOUVEAU : BONUS KILLER HEURISTIC
            if Some(*m) == killers[0] {
                score += 20000; // Priorité absolue (Testé juste après le coup de la Table de Transposition)
            } else if Some(*m) == killers[1] {
                score += 15000;
            }
            
            // Reverse(score) permet de trier par ordre décroissant (du plus grand au plus petit)
            std::cmp::Reverse(score)
        });
    }



    /// ---- MiniMax ----


    fn minimax(&mut self, state: GameState, depth: u8, ply: usize, mut alpha: f32, mut beta: f32, is_maximizer: bool) -> (f32, Option<Move>) {
        self.nodes_explored += 1;
        let orig_alpha = alpha;
        let orig_beta = beta;


        // Gestion du temps 
        if (self.nodes_explored & 2047) == 0 {
            self.check_time();
        }
        if self.abort_search {
            return (0.0, None); 
        }


        // Sécurité pour ne pas déborder du tableau (
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


        // Conditions d'arrêt
        let finished = state.is_game_finished();
        if finished == 0 { return (10000.0 + depth as f32, None); }
        if finished == 1 { return (-10000.0 - depth as f32, None); }
        

        // --- Quiescence Search ---
        if depth == 0 { 
            let q_score = self.quiescence(state, alpha, beta, is_maximizer, ply, 0);
            return (q_score, None); 
        }



        // Génération et Tri des Coups
        let killers = self.killer_moves[safe_ply];
        let mut moves = Self::get_all_moves(&state, state.player_to_move);
        

        // Si la TableTransposition nous suggère un coup, on le place en premier (O(1) avec swap)
                
        if let Some(tm) = tt_move {
            if let Some(pos) = moves.iter().position(|&m| m == tm) {
                moves.swap(0, pos);
            }
        } else {
            Self::sort_moves(&state, &mut moves, &killers);
        }


        

        let mut best_move = None;
        let mut best_score = if is_maximizer { f32::NEG_INFINITY } else { f32::INFINITY };

        //  Boucle Principale
        for (i, m) in moves.iter().enumerate() {
            let child_state = state.apply_move(m.move_type, m.x, m.y);

            if m.move_type != 0 {
                if !child_state.is_state_valid() { continue; }
            }

            let mut value: f32;
            let is_late_move = i >= 4;
            let is_wall = m.move_type != 0;

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
                    // Coupure + mémorisation du killer Move
                    if self.killer_moves[safe_ply][0] != Some(*m) {
                        self.killer_moves[safe_ply][1] = self.killer_moves[safe_ply][0]; // Décale l'ancien #1 en #2
                        self.killer_moves[safe_ply][0] = Some(*m); // Enregistre le nouveau #1
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
                    // Coupure + mémorisation du killer Move
                    if self.killer_moves[safe_ply][0] != Some(*m) {
                        self.killer_moves[safe_ply][1] = self.killer_moves[safe_ply][0];
                        self.killer_moves[safe_ply][0] = Some(*m);
                    }
                    break; 
                }
            }
        }



        //  Écriture dans la Table de Transposition
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



    /// Recherche de Stabilité (Quiescence Search)
    /// Résout l'Effet d'Horizon en prolongeant la recherche sur les coups violents
    fn quiescence(&mut self, state: GameState, mut alpha: f32, mut beta: f32, is_maximizer: bool, ply: usize, q_depth: u8) -> f32 {
        self.nodes_explored += 1;

        return self.evaluate(&state);


        // Gestion du temps 
        if (self.nodes_explored & 2047) == 0 {
            self.check_time();
        }
        if self.abort_search {
            return 0.0;
        }


        let finished = state.is_game_finished();
        if finished == 0 { return 10000.0; }
        if finished == 1 { return -10000.0; }
        
        // Limite stricte
        if q_depth >= 0 { return self.evaluate(&state); } // ---- Limite à partir de laquelle on stop le quiescence search ----

        // L'évaluation "Stand Pat" (On suppose qu'on peut toujours ne rien faire)
        let stand_pat = self.evaluate(&state);

        if is_maximizer {
            if stand_pat >= beta { return beta; } // Coupure Beta immédiate
            alpha = alpha.max(stand_pat);
        } else {
            if stand_pat <= alpha { return alpha; } // Coupure Alpha immédiate
            beta = beta.min(stand_pat);
        }


        let mut moves = Self::generate_pawn_moves(&state, state.player_to_move);
        let safe_ply = ply.min(127);
        
        if state.walls_left[state.player_to_move as usize] > 0 {
            let legal_walls = Self::generate_legal_walls(&state, state.player_to_move);
            
            if let Some(k1) = self.killer_moves[safe_ply][0] {
                if k1.move_type != 0 && legal_walls.contains(&k1) { moves.push(k1); } // vérifie que le killer move ne soit pas un déplacement vers un mur 
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

            // Vérification anti-enfermement pour les murs tueurs
            if m.move_type != 0 {
                if !child_state.is_state_valid() { continue; }
            }

            // Appel récursif de la quiescence (q_depth augmente)
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

    /// Vide le cache (utile si on relance une nouvelle partie)
    pub fn clear_cache(&mut self) {
        self.tt.clear();
        self.killer_moves = [[None; 2]; 128];
    }

    /// Le point d'entrée officiel de l'IA
    pub fn get_best_move(&mut self, state: &GameState, max_depth: u8, time_limit_ms: u64, player_to_maximise: u8) -> Option<Move> {
        self.nodes_explored = 0;
        
        // Démarrage de la montre
        self.start_time = Some(Instant::now());
        self.time_limit_ms = time_limit_ms;
        self.abort_search = false;

        let mut overall_best_action = None;

        // Iterative Deepening
        for d in 1..=max_depth {
            self.killer_moves = [[None; 2]; 128]; 
            
            let (_, m) = self.minimax(*state, d, 0, f32::NEG_INFINITY, f32::INFINITY, state.player_to_move == player_to_maximise);
            

            if self.abort_search { // ne sauvegarde pas si on a abandonné la recherche 
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

/// Table de transposition
#[derive(Clone, Copy)]
pub struct TTEntry {
    pub depth: u8,
    pub score: f32,
    pub flag: u8,
    pub best_move: Option<Move>,
}



// L'initialisation du module
#[pymodule]
fn barricade_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<GameState>()?;
    m.add_class::<Move>()?; 
    m.add_class::<Engine>()?;
    m.add_class::<EvalWeights>()?; 
    Ok(())
}