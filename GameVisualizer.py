import arcade
import kuyper 
from arcade.types import Color
from buttons import Button
import multiprocessing



def ai_worker_target(move_history, queue, player_to_maximise):
    """
    Tourne dans un processus séparé. 
    On reconstruit l'état depuis l'historique pour éviter les crashs de sérialisation PyO3.
    """
    state = kuyper.GameState()
    for m_type, mx, my in move_history:
        state = state.apply_move(m_type, mx, my)


    engine = kuyper.Engine()

    total_walls_left = state.walls_left[0] + state.walls_left[1]
    dynamic_depth = 8
    if total_walls_left <= 12:
        dynamic_depth = 9
    if total_walls_left <= 6:
        dynamic_depth = 10
    if total_walls_left <= 2:
        dynamic_depth = 12 # C'est une pure course ou un dernier blocage, on lit l'avenir !
    

    best_move = engine.get_best_move(state, max_depth=40, time_limit_ms=3000, player_to_maximise=0)

    if best_move:
        queue.put((best_move.move_type, best_move.x, best_move.y))
    else:
        queue.put(None)


class GameVisualizerUtils:
    BOX_PADDING_X = 20
    BOX_PADDING_Y = 20
    BOX_OFFSET_X = 70
    BOX_OFFSET_Y = 70
    BOX_SIZE_X = 50
    BOX_SIZE_Y = 50

    @staticmethod
    def extract_walls_from_bitboard(bitboard):
        """Traduit un u64 de Rust en liste de coordonnées (x,y) pour le dessin"""
        walls = []
        valid = bitboard
        while valid != 0:
            # L'équivalent Python strict de trailing_zeros()
            idx = (valid & -valid).bit_length() - 1
            walls.append((idx % 8, idx // 8))
            valid &= valid - 1
        return walls

    @staticmethod
    def get_visual_box(x, y):
        return (GameVisualizerUtils.BOX_PADDING_X + x * GameVisualizerUtils.BOX_OFFSET_X, 
                GameVisualizerUtils.BOX_PADDING_Y + y * GameVisualizerUtils.BOX_OFFSET_Y,  
                GameVisualizerUtils.BOX_SIZE_X, GameVisualizerUtils.BOX_SIZE_Y)

    @staticmethod
    def get_box_at(x, y):
        for by in range(9):
            for bx in range(9):
                left = GameVisualizerUtils.BOX_PADDING_X + bx * GameVisualizerUtils.BOX_OFFSET_X
                bottom = GameVisualizerUtils.BOX_PADDING_Y + by * GameVisualizerUtils.BOX_OFFSET_Y
                if left < x < left + GameVisualizerUtils.BOX_SIZE_X and bottom < y < bottom + GameVisualizerUtils.BOX_SIZE_Y:
                    return (bx, by)
        return None

    @staticmethod
    def get_visual_box_center(bx, by):
        return (GameVisualizerUtils.BOX_PADDING_X + bx * GameVisualizerUtils.BOX_OFFSET_X + 0.5 * GameVisualizerUtils.BOX_SIZE_X, 
                GameVisualizerUtils.BOX_PADDING_Y + by * GameVisualizerUtils.BOX_OFFSET_Y + 0.5 * GameVisualizerUtils.BOX_SIZE_Y)

    @staticmethod
    def get_wall_at(x, y):
        local_x = x - GameVisualizerUtils.BOX_PADDING_X
        local_y = y - GameVisualizerUtils.BOX_PADDING_Y

        if local_x < 0 or local_y < 0: return None

        grid_x = int(local_x // GameVisualizerUtils.BOX_OFFSET_X)
        grid_y = int(local_y // GameVisualizerUtils.BOX_OFFSET_Y)
        rem_x = local_x % GameVisualizerUtils.BOX_OFFSET_X
        rem_y = local_y % GameVisualizerUtils.BOX_OFFSET_Y

        in_gap_x = rem_x > GameVisualizerUtils.BOX_SIZE_X
        in_gap_y = rem_y > GameVisualizerUtils.BOX_SIZE_Y

        if grid_x > 7 or grid_y > 7: return None
        if in_gap_x and in_gap_y: return None

        # Remplacement des PlayerAction par les types entiers (1=V, 2=H)
        if in_gap_x and not in_gap_y: return (1, (grid_x, grid_y))
        if in_gap_y and not in_gap_x: return (2, (grid_x, grid_y))
        return None

    @staticmethod
    def get_visual_wall_h_lrbt(wx, wy):
        l, b, _, _ = GameVisualizerUtils.get_visual_box(wx, wy) 
        return l, l + GameVisualizerUtils.BOX_OFFSET_X + GameVisualizerUtils.BOX_SIZE_X, b + GameVisualizerUtils.BOX_SIZE_Y, b + GameVisualizerUtils.BOX_OFFSET_Y

    @staticmethod
    def get_visual_wall_v_lrbt(wx, wy):
        l, b, _, _ = GameVisualizerUtils.get_visual_box(wx, wy) 
        return l + GameVisualizerUtils.BOX_SIZE_X, l + GameVisualizerUtils.BOX_OFFSET_X, b, b + (GameVisualizerUtils.BOX_OFFSET_Y + GameVisualizerUtils.BOX_SIZE_Y)


class BarricadeGame(arcade.Window):
    def __init__(self, width, height):
        super().__init__(width, height, "Barricade Solver (Powered by Rust)", resizable=False)
        
        self.state = kuyper.GameState()
        self.move_history = [] # Enregistre les coups pour le Multiprocessing
        self.human_player = 1

        self.ai_computing = False
        self.ai_process = None
        self.ai_queue = multiprocessing.Queue()

        self.loading_timer = 0.0
        self.load_text = arcade.text.Text(f"L'IA réfléchit ...", x=820, y=600, anchor_x="center", color=Color.from_hex_string("#FDFDFD"), font_size=20, bold=True)
        self.load_text_timer = arcade.text.Text(f"0.0s", x=820, y=570, anchor_x="center", color=Color.from_hex_string("#E4E4E4"), font_size=15, bold=True)

        self.player_to_color = {0: "🔴", 1: "🔵"}
        self.victory_text = arcade.text.Text(f"Le joueur a gagné ! ", x=820, y=600, anchor_x="center", color=Color.from_hex_string("#FDFDFD"), font_size=20, bold=True)

        self.text_joueur_rouge = arcade.text.Text(f"Joueur 🔴", x=680, y=400, anchor_x="left", color=Color.from_hex_string("#CFCFCF"), font_size=15, bold=True)
        self.text_barricade_rouge = arcade.text.Text(f"barricades : 10/10", x=680, y=380, anchor_x="left", color=Color.from_hex_string("#CFCFCF"), font_size=13, bold=True)
        
        self.text_joueur_bleu = arcade.text.Text(f"Joueur 🔵", x=680, y=320, anchor_x="left", color=Color.from_hex_string("#CFCFCF"), font_size=15, bold=True)
        self.text_barricade_bleu = arcade.text.Text(f"barricades : 10/10", x=680, y=300, anchor_x="left", color=Color.from_hex_string("#CFCFCF"), font_size=13, bold=True)

        self.reset_button = Button(680, self.width-20, 20, 80, self.reset, arcade.text.Text("Reset", 0, 0, color=Color.from_hex_string("#FFFFFF"), font_size=20, bold=True), Color.from_hex_string("#FF6565"))

        self.is_maj_pressed = False
        self.preview_wall_data = None

    def reset(self, swap_player=False):
        if swap_player:
            self.human_player = 1 - self.human_player

        if self.ai_process is not None and self.ai_process.is_alive():
            self.ai_process.terminate() 
            self.ai_process.join()

        self.ai_queue = multiprocessing.Queue()
        self.ai_process = None
        self.ai_computing = False
        self.loading_timer = 0.0

        self.state = kuyper.GameState()
        self.move_history = []

    def on_draw(self):
        self.clear()
        arcade.set_background_color(Color.from_hex_string("#130820")) 
        arcade.draw_lrbt_rectangle_filled(0, 9*GameVisualizerUtils.BOX_OFFSET_X+20, 0, 9*GameVisualizerUtils.BOX_OFFSET_Y+20, Color.from_hex_string("#D3D9E6"))

        for y in range(9):
            for x in range(9):
                l, b, w, h = GameVisualizerUtils.get_visual_box(x, y)
                arcade.draw_lbwh_rectangle_filled(l, b, w, h, Color.from_hex_string("#F1F1F1"))
                if x < 8 and y < 8:
                    arcade.draw_point(l + 0.5*GameVisualizerUtils.BOX_OFFSET_X + 0.5 * GameVisualizerUtils.BOX_SIZE_X, b + 0.5*GameVisualizerUtils.BOX_OFFSET_Y + 0.5 * GameVisualizerUtils.BOX_SIZE_Y, Color.from_hex_string("#272525"), 2)
                    
        # Mouvements légaux
        if self.state.player_to_move == self.human_player and self.state.is_game_finished() == -1:
            moves = kuyper.Engine.get_all_moves(self.state, self.human_player)
            for m in moves:
                if m.move_type == 0:
                    l, b, w, h = GameVisualizerUtils.get_visual_box(m.x, m.y)
                    arcade.draw_lbwh_rectangle_outline(l, b, w, h, Color.from_hex_string("#4FFC5D"), border_width=2)

        # Joueurs
        p0_idx, p1_idx = self.state.positions
        
        # Décodage Index 1D vers 2D
        cx, cy = GameVisualizerUtils.get_visual_box_center(p0_idx % 9, p0_idx // 9)
        arcade.draw_circle_filled(cx, cy, 15, Color.from_hex_string("#E22222"))
        
        cx, cy = GameVisualizerUtils.get_visual_box_center(p1_idx % 9, p1_idx // 9)
        arcade.draw_circle_filled(cx, cy, 15, Color.from_hex_string("#1973DA"))

        # Murs
        wallColor = Color.from_hex_string("#505050")
        for wx, wy in GameVisualizerUtils.extract_walls_from_bitboard(self.state.walls_h): 
            wl, wr, wb, wt = GameVisualizerUtils.get_visual_wall_h_lrbt(wx, wy)
            arcade.draw_lrbt_rectangle_filled(wl, wr, wb, wt, wallColor)

        for wx, wy in GameVisualizerUtils.extract_walls_from_bitboard(self.state.walls_v): 
            wl, wr, wb, wt = GameVisualizerUtils.get_visual_wall_v_lrbt(wx, wy)
            arcade.draw_lrbt_rectangle_filled(wl, wr, wb, wt, wallColor)

        # Preview Mur
        previewWallColor = Color.from_hex_string("#838383")
        if self.is_maj_pressed and self.preview_wall_data != None: 
            w_type, (wx, wy) = self.preview_wall_data
            if w_type == 2:
                wl, wr, wb, wt = GameVisualizerUtils.get_visual_wall_h_lrbt(wx, wy)
                arcade.draw_lrbt_rectangle_filled(wl, wr, wb, wt, previewWallColor)
            else: 
                wl, wr, wb, wt = GameVisualizerUtils.get_visual_wall_v_lrbt(wx, wy)
                arcade.draw_lrbt_rectangle_filled(wl, wr, wb, wt, previewWallColor)

        self.draw_UI()

    def draw_UI(self):
        if self.ai_computing:
            dots_count = int((self.loading_timer * 2) % 4)
            self.load_text.text = f"L'IA réfléchit{'.' * dots_count}"
            self.load_text.draw()
            self.load_text_timer.text = f"{self.loading_timer:.1f}s"
            self.load_text_timer.draw()

        winner = self.state.is_game_finished()
        if winner != -1:
            self.victory_text.text = f"Le joueur {self.player_to_color[winner]} a gagné ! "
            self.victory_text.draw()

        w0, w1 = self.state.walls_left
        self.text_joueur_rouge.draw()
        self.text_barricade_rouge.text = f"barricades : {w0}/10"
        self.text_barricade_rouge.draw()

        self.text_joueur_bleu.draw()  
        self.text_barricade_bleu.text = f"barricades : {w1}/10"
        self.text_barricade_bleu.draw()

        self.reset_button.draw()

    def on_key_press(self, symbol, modifiers):
        if symbol == arcade.key.LSHIFT or symbol == arcade.key.RSHIFT:
            self.is_maj_pressed = True

    def on_key_release(self, symbol, modifiers):
        if symbol == arcade.key.LSHIFT or symbol == arcade.key.RSHIFT:
            self.is_maj_pressed = False

    def on_mouse_press(self, x, y, button, modifiers):
        self.reset_button.update(x,y)

        if self.state.player_to_move != self.human_player or self.state.is_game_finished() != -1:
            return
        
        valid_moves = kuyper.Engine.get_all_moves(self.state, self.human_player)

        if button == arcade.MOUSE_BUTTON_LEFT and not self.is_maj_pressed:
            box = GameVisualizerUtils.get_box_at(x, y)
            if box != None:
                mx, my = box
                for m in valid_moves:
                    if m.move_type == 0 and m.x == mx and m.y == my:
                        self.state = self.state.apply_move(0, mx, my)
                        self.move_history.append((0, mx, my))
                        break

        elif button == arcade.MOUSE_BUTTON_LEFT and self.is_maj_pressed: 
            wall_data = GameVisualizerUtils.get_wall_at(x, y)
            if wall_data != None:
                w_type, (wx, wy) = wall_data
                for m in valid_moves:
                    if m.move_type == w_type and m.x == wx and m.y == wy:
                        self.state = self.state.apply_move(w_type, wx, wy)
                        self.move_history.append((w_type, wx, wy))
                        break

    def on_mouse_motion(self, x, y, dx, dy):
        if self.state.player_to_move == self.human_player and self.is_maj_pressed:
            self.preview_wall_data = GameVisualizerUtils.get_wall_at(x, y)

    def on_update(self, delta_time):
        if self.ai_computing:
            self.loading_timer += delta_time

        if self.state.player_to_move != self.human_player and self.state.is_game_finished() == -1:
            if not self.ai_computing:
                self.ai_computing = True
                self.loading_timer = 0
                
                # On passe l'historique au lieu du state Rust directement
                self.ai_process = multiprocessing.Process(
                    target=ai_worker_target, 
                    args=(self.move_history, self.ai_queue, self.state.player_to_move)
                )
                self.ai_process.start()

            elif self.ai_process is not None and not self.ai_process.is_alive():
                if not self.ai_queue.empty():
                    ai_action = self.ai_queue.get()
                    if ai_action is not None:
                        m_type, mx, my = ai_action
                        self.state = self.state.apply_move(m_type, mx, my)
                        self.move_history.append((m_type, mx, my))
                
                self.ai_computing = False
                self.ai_process = None


if __name__ == '__main__':
    game = BarricadeGame(1000, 660)
    game.run()