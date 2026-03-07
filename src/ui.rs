#![allow(non_upper_case_globals)] // 禁用全局变量大写警告，保护眼睛

use std::{any::{Any, TypeId}, collections::HashMap, sync::{Mutex, OnceLock}};

slint::include_modules!();


static MANAGER: OnceLock<Mutex<ViewManager>> = OnceLock::new();

// 2. 增加 Send + Sync 约束给 Any (可选，视具体报错而定)
pub struct ViewManager {
    // 明确告诉编译器，里面存的东西是可以在线程间移动的
    pages: HashMap<TypeId, Box<dyn Any + Send>>, 
}

impl ViewManager {
    pub fn new() -> Self {
        Self { pages: HashMap::new() }
    }

    pub fn get_static<T: 'static + Default + Send + Sync>(&mut self) -> &'static T {
        let type_id = TypeId::of::<T>();

        if !self.pages.contains_key(&type_id) {
            let instance = T::default();
            let leaked_ref: &'static T = Box::leak(Box::new(instance));
            self.pages.insert(type_id, Box::new(leaked_ref));
        }

        let ptr_in_map = self.pages.get(&type_id).unwrap();
        *ptr_in_map.downcast_ref::<&'static T>().unwrap()
    }
}


pub fn use_view<T: 'static + Default + Send + Sync>() -> &'static T {
    let mut manager = MANAGER.get_or_init(|| Mutex::new(ViewManager::new()))
        .lock()
        .expect("Failed to lock ViewManager");
    
    let result = manager.get_static::<T>();
    result
}