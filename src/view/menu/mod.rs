use std::{sync::atomic::{AtomicUsize, Ordering}, time::Duration, usize};

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
static ACTIVE_MENU_COUNT: AtomicUsize = AtomicUsize::new(1);



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
    let x = if parent_x + MENU_WIDTH * 3 <= wa_right { parent_x + MENU_WIDTH } else { (parent_x - MENU_WIDTH).max(wa_left) };
    let y = parent_y + offset_y.round() as i32;
    Some((x, y))
}

pub fn close_menus(level: usize) {
    match level {
        1 => {
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
        slint::spawn_local(async move {

            if window_weak.upgrade().unwrap().window().winit_window().await.is_err() {
                return;
            }

            window_weak.upgrade().unwrap().window().on_winit_window_event(|_, event| {
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
        })
        .unwrap();
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
