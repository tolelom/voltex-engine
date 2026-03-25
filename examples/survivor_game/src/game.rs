use voltex_platform::InputState;
use crate::player::Player;

pub struct GameState {
    pub player: Player,
    pub score: u32,
    pub wave: u32,
    pub game_over: bool,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            player: Player::new(),
            score: 0,
            wave: 1,
            game_over: false,
        }
    }

    pub fn update(&mut self, input: &InputState, dt: f32) {
        if self.game_over {
            return;
        }
        self.player.update(input, dt);
    }
}
