import arcade









class Button ():

    def __init__(self, l, r, b, t, called_function, draw_text : arcade.Text, background_color):
        self.l = l
        self.r = r
        self.b = b
        self.t = t
        self.called_function = called_function


        draw_text.anchor_x = "center"
        draw_text.anchor_y = "center"
        draw_text.x = (self.l + self.r) / 2
        draw_text.y = (self.b + self.t) / 2

        self.draw_text = draw_text
        self.background_color = background_color


    def is_in_bondingBox(self, x,y) :
        return self.l < x < self.r and self.b < y < self.t


    def update(self, click_x, click_y) :
        if self.is_in_bondingBox(click_x, click_y) :
            self.called_function()


    def draw(self) :
        arcade.draw_lrbt_rectangle_filled(self.l, self.r, self.b, self.t, self.background_color)
        self.draw_text.draw()


    


# A finir plus tard
class Toggle ():

    def __init__(self, l, r, b, t, called_function, active_color, inactive_color):
        self.l = l
        self.r = r
        self.b = b
        self.t = t
        self.called_function = called_function


        self.active_color = active_color
        self.active_color = inactive_color


    def is_in_bondingBox(self, x,y) :
        return self.l < x < self.r and self.b < y < self.t


    def update(self, click_x, click_y) :
        if self.is_in_bondingBox(click_x, click_y) :
            self.called_function()


    def draw(self) :
        arcade.draw_lrbt_rectangle_filled(self.l, self.r, self.b, self.t, self.background_color)
        self.draw_text.draw()