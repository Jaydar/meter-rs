#![allow(non_upper_case_globals)]

use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
};

slint::include_modules!();

thread_local! {
    static MANAGER: RefCell<ViewManager> = RefCell::new(ViewManager::new());
}

pub struct ViewManager {
    pages: HashMap<TypeId, Box<dyn Any>>,
}

impl ViewManager {
    pub fn new() -> Self {
        Self {
            pages: HashMap::new(),
        }
    }

    pub fn get_static<T: 'static + Default>(&mut self) -> &'static T {
        let type_id = TypeId::of::<T>();

        self.pages.entry(type_id).or_insert_with(|| {
            let leaked_ref: &'static T = Box::leak(Box::new(T::default()));
            Box::new(leaked_ref)
        });

        self.pages
            .get(&type_id)
            .unwrap()
            .downcast_ref::<&'static T>()
            .copied()
            .unwrap()
    }
}

pub fn use_view<T: 'static + Default>() -> &'static T {
    MANAGER.with(|manager| manager.borrow_mut().get_static::<T>())
}
