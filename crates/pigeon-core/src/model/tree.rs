//! Finding and changing items in a collection's tree.

use super::*;

pub fn find_request<'a>(items: &'a [Item], id: &str) -> Option<&'a Request> {
    for it in items {
        match it {
            Item::Request(r) if r.id == id => return Some(r),
            Item::Folder(f) => {
                if let Some(r) = find_request(&f.items, id) {
                    return Some(r);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn find_request_mut<'a>(items: &'a mut [Item], id: &str) -> Option<&'a mut Request> {
    for it in items {
        match it {
            Item::Request(r) if r.id == id => return Some(r),
            Item::Folder(f) => {
                if let Some(r) = find_request_mut(&mut f.items, id) {
                    return Some(r);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn find_folder_mut<'a>(items: &'a mut [Item], id: &str) -> Option<&'a mut Folder> {
    for it in items {
        if let Item::Folder(f) = it {
            if f.id == id {
                return Some(f);
            }
            if let Some(x) = find_folder_mut(&mut f.items, id) {
                return Some(x);
            }
        }
    }
    None
}

/// Find the item and return a mutable ref to the list holding it, plus its index.
pub fn find_parent_list_mut<'a>(items: &'a mut Vec<Item>, id: &str) -> Option<(&'a mut Vec<Item>, usize)> {
    if let Some(i) = items.iter().position(|it| it.id() == id) {
        return Some((items, i));
    }
    for it in items.iter_mut() {
        if let Item::Folder(f) = it
            && let Some(x) = find_parent_list_mut(&mut f.items, id)
        {
            return Some(x);
        }
    }
    None
}

/// The auth chain for a request: folder auths from innermost to outermost, then collection.
pub fn auth_chain(items: &[Item], id: &str, acc: &mut Vec<Auth>) -> bool {
    for it in items {
        match it {
            Item::Request(r) if r.id == id => return true,
            Item::Folder(f) if auth_chain(&f.items, id, acc) => {
                acc.push(f.auth.clone());
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Scripts of the folders around a request, from the innermost folder outwards.
pub fn script_chain(items: &[Item], id: &str, acc: &mut Vec<Scripts>) -> bool {
    for it in items {
        match it {
            Item::Request(r) if r.id == id => return true,
            Item::Folder(f) if script_chain(&f.items, id, acc) => {
                acc.push(f.scripts.clone());
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Folder/request names leading to a request (for breadcrumbs).
pub fn path_to(items: &[Item], id: &str, acc: &mut Vec<String>) -> bool {
    for it in items {
        match it {
            Item::Request(r) if r.id == id => return true,
            Item::Folder(f) => {
                if f.id == id {
                    return true;
                }
                if path_to(&f.items, id, acc) {
                    acc.insert(0, f.name.clone());
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

pub fn count_requests(items: &[Item]) -> usize {
    items
        .iter()
        .map(|i| match i {
            Item::Request(_) => 1,
            Item::Folder(f) => count_requests(&f.items),
        })
        .sum()
}

/// Deep clone of an item tree with new ids.
pub fn reid(item: &Item) -> Item {
    match item {
        Item::Request(r) => {
            let mut r = r.clone();
            r.id = new_id();
            Item::Request(r)
        }
        Item::Folder(f) => {
            let mut f = f.clone();
            f.id = new_id();
            f.items = f.items.iter().map(reid).collect();
            Item::Folder(f)
        }
    }
}
