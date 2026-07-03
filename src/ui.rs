use std::{
    any::TypeId,
    cell::RefCell,
    collections::HashMap,
};

use anyhow::Context;

use crate::view::ViewTrait;

slint::include_modules!();

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => Self::System,
        }
    }
}

impl SnapMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkArea => "work_area",
            Self::FullScreen => "full_screen",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "full_screen" => Self::FullScreen,
            _ => Self::WorkArea,
        }
    }
}

impl DisplayMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Simple => "simple",
            Self::Taskbar => "taskbar",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "simple" => Self::Simple,
            "taskbar" => Self::Taskbar,
            _ => Self::Normal,
        }
    }
}

thread_local! {
    static _manager: RefCell<ViewManager> = RefCell::new(ViewManager::new());
}

pub struct ViewManager {
    pages: HashMap<TypeId, &'static dyn ViewTrait>,
}

impl ViewManager {
    pub fn new() -> Self {
        Self { pages: HashMap::new() }
    }

    pub fn get_static<T: 'static + ViewTrait>(&mut self) -> &'static T {
        let type_id = TypeId::of::<T>();
        let page = *self.pages.entry(type_id).or_insert_with(|| Box::leak(Box::new(T::new())) as &'static dyn ViewTrait);
        match page.as_any().downcast_ref::<T>().context("view manager stored a different type for this TypeId") {
            Ok(page) => page,
            Err(err) => panic!("{}", err),
        }
    }
}

pub fn use_view<T: 'static + ViewTrait>() -> &'static T {
    _manager.with(|manager_cell| manager_cell.borrow_mut().get_static::<T>())
}
