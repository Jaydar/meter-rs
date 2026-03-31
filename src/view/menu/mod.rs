use anyhow::Context;
use std::{sync::atomic::{AtomicUsize, Ordering}, time::Duration, usize};
use tracing::error;

use slint::{ComponentHandle, Timer};

use i_slint_backend_winit::{EventResult, WinitWindowAccessor};
use winit::event::WindowEvent;

use crate::{MAIN_HWND, tools, trim_memory, ui, view::ViewTrait};

mod menu1_view;
pub use menu1_view::Menu1View;

mod menu2_view;
pub use menu2_view::Menu2View;

mod menu3_view;
pub use menu3_view::Menu3View;


pub const MENU_WIDTH: i32 = 180;
pub const MENU_GAP: i32 = 2;
static ACTIVE_MENU_COUNT: AtomicUsize = AtomicUsize::new(1);



pub fn calc_menu_show_position(level: i32, root_pos_x: f32, parent_pos_y: f32, offset_y: f32) -> Option<(i32, i32)> {
    let hwnd = MAIN_HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return None;
    }
    let Some((wa_left, _wa_top, wa_right, _wa_bottom)) = tools::get_work_area(hwnd) else {
        return None;
    };
    let root_x = root_pos_x.round() as i32;
    let parent_y = parent_pos_y.round() as i32;
    let x = if root_x + MENU_WIDTH * level + MENU_GAP * (level - 1) <= wa_right {
        root_x + (MENU_WIDTH + MENU_GAP) * (level - 1)
    } else {
        (root_x - (MENU_WIDTH + MENU_GAP) * (level - 1)).max(wa_left)
    };
    let y = parent_y + offset_y.round() as i32;
    Some((x, y))
}

pub fn close_menus(level: usize) {
    match level {
        1 => {
            ui::use_view::<Menu1View>().ui.set_active_submenu(-1);
            ui::use_view::<Menu1View>().hide();

            ui::use_view::<Menu2View>().hide();
            ui::use_view::<Menu3View>().hide();
            trim_memory()
        }
        2 => {
            ui::use_view::<Menu2View>().hide();
            ui::use_view::<Menu3View>().hide();
        }
        3 => {
            ui::use_view::<Menu3View>().hide();
        }
        _ => {}
    }
}

pub fn listen_menu_close() {
    fn bind_close_event<T: ComponentHandle + 'static>(window_weak: slint::Weak<T>) {
        let result = slint::spawn_local(async move {
            let Some(window) = window_weak.upgrade().context("upgrade menu weak failed").map_err(|err| error!("{}", err)).ok() else {
                return;
            };
            if window.window().winit_window().await.is_err() {
                return;
            }

            window.window().on_winit_window_event(|_, event| {
                match event {
                    WindowEvent::Focused(true) =>{
                        ACTIVE_MENU_COUNT.fetch_add(1, Ordering::SeqCst);
                    }
                    WindowEvent::Focused(false) => {
                        ACTIVE_MENU_COUNT.fetch_sub(1, Ordering::SeqCst);
                        Timer::single_shot(Duration::from_millis(100), || {
                            if ACTIVE_MENU_COUNT.load(Ordering::SeqCst) <= 0 {
                                close_menus(1);
                            }
                        });
                    }
                    _ => {}
                }

                EventResult::Propagate
            });
        });
        if let Err(err) = result {
            error!("spawn menu close listener failed: {}", err);
        }
    }

    let bind_menus: [fn(); 3] = [
        || bind_close_event(ui::use_view::<Menu1View>().ui.as_weak()),
        || bind_close_event(ui::use_view::<Menu2View>().ui.as_weak()),
        || bind_close_event(ui::use_view::<Menu3View>().ui.as_weak()),
    ];

    for bind_menu in bind_menus {
        bind_menu();
    }
}
