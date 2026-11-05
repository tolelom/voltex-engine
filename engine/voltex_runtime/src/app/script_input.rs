//! Bridges the platform [`InputState`] to the windowing-agnostic
//! [`voltex_script::ScriptInput`] snapshot that Lua scripts read.

use voltex_platform::InputState;

/// Mapping from the string names exposed to Lua (`input.is_key_pressed("W")`)
/// to platform keycodes. Owning the table here keeps `voltex_script` free of
/// any windowing/winit dependency. Unlisted keys are simply unqueryable.
const SCRIPT_KEY_NAMES: &[(&str, winit::keyboard::KeyCode)] = {
    use winit::keyboard::KeyCode as K;
    &[
        ("A", K::KeyA), ("B", K::KeyB), ("C", K::KeyC), ("D", K::KeyD),
        ("E", K::KeyE), ("F", K::KeyF), ("G", K::KeyG), ("H", K::KeyH),
        ("I", K::KeyI), ("J", K::KeyJ), ("K", K::KeyK), ("L", K::KeyL),
        ("M", K::KeyM), ("N", K::KeyN), ("O", K::KeyO), ("P", K::KeyP),
        ("Q", K::KeyQ), ("R", K::KeyR), ("S", K::KeyS), ("T", K::KeyT),
        ("U", K::KeyU), ("V", K::KeyV), ("W", K::KeyW), ("X", K::KeyX),
        ("Y", K::KeyY), ("Z", K::KeyZ),
        ("0", K::Digit0), ("1", K::Digit1), ("2", K::Digit2), ("3", K::Digit3),
        ("4", K::Digit4), ("5", K::Digit5), ("6", K::Digit6), ("7", K::Digit7),
        ("8", K::Digit8), ("9", K::Digit9),
        ("Space", K::Space), ("Enter", K::Enter), ("Escape", K::Escape),
        ("Tab", K::Tab),
        ("Left", K::ArrowLeft), ("Right", K::ArrowRight),
        ("Up", K::ArrowUp), ("Down", K::ArrowDown),
        ("LShift", K::ShiftLeft), ("RShift", K::ShiftRight),
        ("LCtrl", K::ControlLeft), ("RCtrl", K::ControlRight),
        ("LAlt", K::AltLeft), ("RAlt", K::AltRight),
    ]
};

/// Translate the platform [`InputState`] into the windowing-agnostic
/// [`voltex_script::ScriptInput`] snapshot read by Lua.
pub(super) fn fill_script_input(script_input: &mut voltex_script::ScriptInput, input: &InputState) {
    use winit::event::MouseButton;
    script_input.clear();
    for (name, code) in SCRIPT_KEY_NAMES {
        if input.is_key_pressed(*code) {
            script_input.set_key_pressed(name);
        }
        if input.is_key_just_pressed(*code) {
            script_input.set_key_just_pressed(name);
        }
    }
    let (mx, my) = input.mouse_position();
    let (dx, dy) = input.mouse_delta();
    script_input.set_mouse(mx, my, dx, dy);
    script_input.set_buttons(
        input.is_mouse_button_pressed(MouseButton::Left),
        input.is_mouse_button_pressed(MouseButton::Right),
        input.is_mouse_button_pressed(MouseButton::Middle),
    );
    script_input.set_scroll(input.mouse_scroll());
}
