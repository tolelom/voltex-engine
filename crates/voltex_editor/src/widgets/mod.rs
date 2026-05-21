mod controls;
mod containers;
mod text_input;
mod drag;

#[cfg(test)]
mod tests {
    use crate::ui_context::{Key, UiContext};

    #[test]
    fn test_button_returns_false_when_not_clicked() {
        let mut ctx = UiContext::new(800.0, 600.0);
        // Mouse is at (500, 500) — far from any button
        ctx.begin_frame(500.0, 500.0, false);
        let result = ctx.button("Click Me");
        assert!(!result, "button should return false when mouse is not over it");
    }

    #[test]
    fn test_button_returns_true_when_clicked() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: mouse over button, pressed down
        // Button will be at layout cursor (0, 0) with some width/height
        // glyph_width=8, "OK"=2 chars, btn_w = 2*8 + 4*2 = 24, btn_h = 12 + 4*2 = 20
        ctx.begin_frame(10.0, 5.0, true);
        let _ = ctx.button("OK");

        // Frame 2: mouse still over button, released
        ctx.begin_frame(10.0, 5.0, false);
        let result = ctx.button("OK");
        assert!(result, "button should return true when clicked and released");
    }

    #[test]
    fn test_slider_returns_clamped_value() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.begin_frame(0.0, 0.0, false);

        // Value above max should be clamped
        let v = ctx.slider("test", 150.0, 0.0, 100.0);
        assert!((v - 100.0).abs() < 1e-6, "slider should clamp to max: got {}", v);

        ctx.begin_frame(0.0, 0.0, false);
        // Value below min should be clamped
        let v2 = ctx.slider("test", -10.0, 0.0, 100.0);
        assert!((v2 - 0.0).abs() < 1e-6, "slider should clamp to min: got {}", v2);
    }

    // ── Text Input Tests ──────────────────────────────────────────────

    #[test]
    fn test_text_input_basic_typing() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::new();

        // Click on the text input to focus it (at x=10, y=10, width=200)
        ctx.begin_frame(15.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Now type some characters
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_char('H');
        ctx.input_char('i');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "Hi");
    }

    #[test]
    fn test_text_input_backspace() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("abc");

        // Focus — click far right so cursor goes to end (padding=4, gw=8, 3 chars → need x > 10+4+24=38)
        ctx.begin_frame(50.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Backspace
        ctx.begin_frame(50.0, 15.0, false);
        ctx.input_key(Key::Backspace);
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "ab");
    }

    #[test]
    fn test_text_input_cursor_movement() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("abc");

        // Focus — click far right so cursor goes to end
        ctx.begin_frame(50.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert_eq!(ctx.cursor_pos, 3); // cursor at end of "abc"

        // Move cursor to beginning with Home
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_key(Key::Home);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert_eq!(ctx.cursor_pos, 0);

        // Type 'X' at beginning
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_char('X');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "Xabc");
        assert_eq!(ctx.cursor_pos, 1);
    }

    #[test]
    fn test_text_input_delete_key() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("abc");

        // Focus
        ctx.begin_frame(15.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Move to Home, then Delete
        ctx.begin_frame(15.0, 15.0, false);
        ctx.input_key(Key::Home);
        ctx.input_key(Key::Delete);
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "bc");
    }

    #[test]
    fn test_text_input_arrow_keys() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("hello");

        // Focus — click far right so cursor at end
        ctx.begin_frame(100.0, 15.0, true);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);

        // Left twice from end (pos 5→3)
        ctx.begin_frame(100.0, 15.0, false);
        ctx.input_key(Key::Left);
        ctx.input_key(Key::Left);
        ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert_eq!(ctx.cursor_pos, 3);

        // Type 'X' at position 3
        ctx.begin_frame(100.0, 15.0, false);
        ctx.input_char('X');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(changed);
        assert_eq!(buf, "helXlo");
    }

    #[test]
    fn test_text_input_no_change_when_not_focused() {
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut buf = String::from("test");

        // Don't click on the input — mouse at (500, 500) far away
        ctx.begin_frame(500.0, 500.0, true);
        ctx.input_char('X');
        let changed = ctx.text_input(1, &mut buf, 10.0, 10.0, 200.0);
        assert!(!changed);
        assert_eq!(buf, "test");
    }

    // ── Scroll Panel Tests ────────────────────────────────────────────

    #[test]
    fn test_scroll_offset_clamping() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Panel at (0,0), 200x100, content_height=300
        // Scroll down a lot
        ctx.begin_frame(100.0, 50.0, false);
        ctx.set_scroll(-100.0); // scroll down
        ctx.begin_scroll_panel(1, 0.0, 0.0, 200.0, 100.0, 300.0);
        ctx.end_scroll_panel();

        // max_scroll = 300 - 100 = 200; scroll should be clamped
        let scroll = ctx.scroll_offsets.get(&1).copied().unwrap_or(0.0);
        assert!((0.0..=200.0).contains(&scroll), "scroll={}", scroll);
    }

    #[test]
    fn test_scroll_offset_does_not_go_negative() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Scroll up when already at top
        ctx.begin_frame(100.0, 50.0, false);
        ctx.set_scroll(100.0); // scroll up
        ctx.begin_scroll_panel(1, 0.0, 0.0, 200.0, 100.0, 300.0);
        ctx.end_scroll_panel();

        let scroll = ctx.scroll_offsets.get(&1).copied().unwrap_or(0.0);
        assert!((scroll - 0.0).abs() < 1e-6, "scroll should be 0, got {}", scroll);
    }

    #[test]
    fn test_scroll_panel_content_clipping() {
        let mut ctx = UiContext::new(800.0, 600.0);

        ctx.begin_frame(100.0, 50.0, false);
        ctx.begin_scroll_panel(1, 10.0, 20.0, 200.0, 100.0, 300.0);

        // Draw some content inside
        ctx.text("Inside scroll");
        // Commands drawn inside should have a scissor rect
        let has_scissor = ctx.draw_list.commands.iter().any(|c| c.scissor.is_some());
        assert!(has_scissor, "commands inside scroll panel should have scissor rects");

        ctx.end_scroll_panel();

        // Commands drawn after end_scroll_panel should NOT have scissor
        let cmds_before = ctx.draw_list.commands.len();
        ctx.text("Outside scroll");
        let new_cmds = &ctx.draw_list.commands[cmds_before..];
        let has_scissor_after = new_cmds.iter().any(|c| c.scissor.is_some());
        assert!(!has_scissor_after, "commands after end_scroll_panel should not have scissor");
    }

    // ── Drag and Drop Tests ──────────────────────────────────────────

    #[test]
    fn test_drag_start_and_end() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: mouse down — begin drag
        ctx.begin_frame(100.0, 100.0, true);
        ctx.begin_drag(1, 42);
        assert!(!ctx.is_dragging(), "should not be dragging yet (below threshold)");
        let _ = ctx.end_drag();

        // Frame 2: mouse moved past threshold, still down
        ctx.begin_frame(110.0, 100.0, true);
        let _ = ctx.end_drag();
        assert!(ctx.is_dragging(), "should be dragging after moving past threshold");

        // Frame 3: mouse released
        ctx.begin_frame(120.0, 100.0, false);
        let result = ctx.end_drag();
        assert!(result.is_some());
        let (src_id, payload) = result.unwrap();
        assert_eq!(src_id, 1);
        assert_eq!(payload, 42);
    }

    #[test]
    fn test_drop_on_target() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: begin drag
        ctx.begin_frame(100.0, 100.0, true);
        ctx.begin_drag(1, 99);
        let _ = ctx.end_drag();

        // Frame 2: move past threshold
        ctx.begin_frame(110.0, 100.0, true);
        let _ = ctx.end_drag();

        // Frame 3: release over drop target at (200, 200, 50, 50)
        ctx.begin_frame(220.0, 220.0, false);
        let drop_result = ctx.drop_target(2, 200.0, 200.0, 50.0, 50.0);
        assert_eq!(drop_result, Some(99));
        let _ = ctx.end_drag();
    }

    #[test]
    fn test_drop_outside_target() {
        let mut ctx = UiContext::new(800.0, 600.0);

        // Frame 1: begin drag
        ctx.begin_frame(100.0, 100.0, true);
        ctx.begin_drag(1, 77);
        let _ = ctx.end_drag();

        // Frame 2: move past threshold
        ctx.begin_frame(110.0, 100.0, true);
        let _ = ctx.end_drag();

        // Frame 3: release far from drop target
        ctx.begin_frame(500.0, 500.0, false);
        let drop_result = ctx.drop_target(2, 200.0, 200.0, 50.0, 50.0);
        assert_eq!(drop_result, None);
    }
}
