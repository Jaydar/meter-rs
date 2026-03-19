use std::sync::atomic::Ordering;

use crate::{tools, MAIN_HWND};

pub const MENU_WIDTH: i32 = 180;

pub fn calc_menu_show_position(parent_pos_x: f32, parent_pos_y: f32, offset_y: f32) -> Option<(i32, i32)> {
    let hwnd = MAIN_HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return None;
    }
    let Some((wa_left, _wa_top, wa_right, _wa_bottom)) = tools::get_work_area(hwnd) else {
        return None;
    };
    let parent_x = parent_pos_x.round() as i32;
    let parent_y = parent_pos_y.round() as i32;
    let x = if parent_x + MENU_WIDTH * 3 <= wa_right {
        parent_x + MENU_WIDTH
    } else {
        (parent_x - MENU_WIDTH).max(wa_left)
    };
    let y = parent_y + offset_y.round() as i32;
    Some((x, y))
}

mod menu3_view;
pub use menu3_view::Menu3View;

mod menu_view;
pub use menu_view::MenuView;

mod menu2_view;
pub use menu2_view::Menu2View;
