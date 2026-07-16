mod app;
use std::any::Any;

pub use app::AppView;

mod about;
pub use about::AboutView;

mod mac_address;
pub use mac_address::MacAddressView;

mod port_proxy;
pub use port_proxy::PortProxyView;

mod route_manager;
pub use route_manager::RouteManagerView;

pub trait ViewTrait {
    fn new() -> Self where Self: Sized;
    fn show(&self, extra: Option<&dyn Any>) -> anyhow::Result<()>;
    fn hide(&self);
    fn close(&self) {
        self.hide();
    }
    fn set_position(&self);
    fn bind_event(self) -> Self where Self: Sized;
    fn as_any(&self) -> &dyn Any;
}
