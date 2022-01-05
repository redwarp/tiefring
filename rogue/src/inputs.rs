use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

pub enum Input {
    Up,
    Down,
    Right,
    Left,
    Escape,
    Space,
}

impl Input {
    pub fn from_input_helper(input_helper: &WinitInputHelper) -> Option<Self> {
        if input_helper.key_pressed(VirtualKeyCode::Up) {
            Some(Input::Up)
        } else if input_helper.key_pressed(VirtualKeyCode::Down) {
            Some(Input::Down)
        } else if input_helper.key_pressed(VirtualKeyCode::Right) {
            Some(Input::Right)
        } else if input_helper.key_pressed(VirtualKeyCode::Left) {
            Some(Input::Left)
        } else if input_helper.key_pressed(VirtualKeyCode::Escape) {
            Some(Input::Escape)
        } else if input_helper.key_pressed(VirtualKeyCode::Space) {
            Some(Input::Space)
        } else {
            None
        }
    }
}
