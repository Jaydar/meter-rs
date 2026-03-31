mod app_view;
use std::any::Any;

pub use app_view::AppView;

mod about_view;
pub use about_view::AboutView;

mod menu;
pub use menu::Menu1View;



pub trait ViewTrait {
    fn new() -> Self where Self: Sized;
    fn show(&self, extra: Option<&dyn Any>) -> anyhow::Result<()>;
    fn hide(&self);
    fn set_position(&self);
    fn bind_event(self) -> Self where Self: Sized;
    fn as_any(&self) -> &dyn Any;
}
