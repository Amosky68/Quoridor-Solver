# Kuyper

**Kuyper** is a solver for the board game Quoridor written in Rust.


## 📥Installation

1. Clone the repo 
2. install the required python packages :
```bash
pip install maturin arcade
```

3. get to the barricade_engine folder and compile the engine :
```bash
cd .\barricade_engine\
maturin develop --release
```

4. run the GameVisualizer.py script 


## 🛠️ How does it work ?

The game of Quoridor can be represented using a unique *GameState*. Given a gameState, the solver runs the Minimax algorithm and outputs the best move it found. 
To optimise compute time, the *GameState* ais represented using only bitmaps. 
This way, the engine can explore around 2.4 million nodes par sec on a i5-11400f cpu.

The engine itself is also optimised to further cut down the number of explored nodes per depth. It uses the following techniques : 
- **Alpha-beta pruning**: Cuts off branches in the search tree that are mathematically proven to be worse than previously evaluated moves, significantly reducing the search space.
- **Transposition tables**: Uses Zobrist Hashing to cache previously evaluated board states. If the engine reaches the same position through a different move order, it retrieves the score instantly instead of recalculating it.
- **Fine-tuned heuristics**: A custom evaluation function optimized using SPSA (Simultaneous Perturbation Stochastic Approximation). It goes beyond simple shortest-path counting by analyzing path immunity, tunnel security, and structural bottlenecks.
- **Iterative deepening**: The search runs repeatedly, increasing the depth limit by one each time. This guarantees a solid move is ready if the time limit is reached, and provides excellent move ordering for the alpha-beta algorithm.
- **Quiescence search**: Solves the "horizon effect". When the target depth is reached, the engine doesn't stop immediately if the position is highly volatile (e.g., a critical wall is about to trap a player). It continues exploring "violent" wall placements until the board state is stable.
- **Killer heuristic**: Improves move ordering by keeping track of moves that caused alpha-beta cutoffs in sibling nodes. Testing these "killer" moves first drastically increases the efficiency of the pruning.
- **Bitwise flood-fill (X-Ray)**: Uses `u128` integer operations to calculate shortest paths and detect chokepoints in mere nanoseconds without any dynamic memory allocations.

