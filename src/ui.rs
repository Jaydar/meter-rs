#![allow(non_upper_case_globals)]

use std::{
    any::TypeId,
    cell::RefCell,
    collections::HashMap,
};

use anyhow::Context;

use crate::view::ViewTrait;

slint::include_modules!();

thread_local! {
    static MANAGER: RefCell<ViewManager> = RefCell::new(ViewManager::new());
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
    MANAGER.with(|manager| manager.borrow_mut().get_static::<T>())
}
