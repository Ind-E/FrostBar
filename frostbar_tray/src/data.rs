use std::sync::{Arc, Mutex};

use crate::{
    item::StatusNotifierItem,
    menu::{MenuDiff, MenuItem, MenuItemUpdate, TrayMenu},
};

type BaseMap = std::collections::HashSet<String>;

#[derive(Debug, Clone)]
pub(crate) struct TrayItemMap {
    inner: Arc<Mutex<BaseMap>>,
}

impl TrayItemMap {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BaseMap::default())),
        }
    }

    pub(crate) fn new_item(&self, dest: String, item: &StatusNotifierItem) {
        let mut lock = self.inner.lock().expect("mutex lock should succeed");
        let _ = item;
        lock.insert(dest);
    }

    pub(crate) fn remove_item(&self, dest: &str) {
        self.inner
            .lock()
            .expect("mutex lock should succeed")
            .remove(dest);
    }

    pub(crate) fn clear_items(&self) -> Vec<String> {
        let mut lock = self.inner.lock().expect("mutex lock should succeed");
        lock.drain().collect()
    }

    pub(crate) fn update_menu(&self, dest: &str, menu: &TrayMenu) {
        let _ = menu;
        let _ = dest;
    }
}

pub fn apply_menu_diffs(tray_menu: &mut TrayMenu, diffs: &[MenuDiff]) {
    let mut diff_iter = diffs.iter().peekable();
    tray_menu.submenus.iter_mut().for_each(|item| {
        if let Some(diff) = diff_iter.next_if(|d| d.id == item.id) {
            apply_menu_item_diff(item, &diff.update);
        }
    });
}

fn apply_menu_item_diff(menu_item: &mut MenuItem, update: &MenuItemUpdate) {
    if let Some(label) = &update.label {
        menu_item.label.clone_from(label);
    }
    if let Some(enabled) = update.enabled {
        menu_item.enabled = enabled;
    }
    if let Some(visible) = update.visible {
        menu_item.visible = visible;
    }
    if let Some(icon_name) = &update.icon_name {
        menu_item.icon_name.clone_from(icon_name);
    }
    if let Some(icon_data) = &update.icon_data {
        menu_item.icon_data.clone_from(icon_data);
    }
    if let Some(toggle_state) = update.toggle_state {
        menu_item.toggle_state = toggle_state;
    }
    if let Some(disposition) = update.disposition {
        menu_item.disposition = disposition;
    }
}
