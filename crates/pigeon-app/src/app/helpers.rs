//! Small helpers for reading the collection tree.

use super::*;

pub(super) fn clean_request(r: &Request) -> Request {
    let mut r = r.clone();
    r.params.retain(|k| !k.is_blank());
    r.headers.retain(|k| !k.is_blank());
    r.body.urlencoded.retain(|k| !k.is_blank());
    r.body.formdata.retain(|k| !k.is_blank());
    r
}

pub(super) fn first_request(items: &[Item]) -> Option<&Request> {
    for it in items {
        match it {
            Item::Request(r) => return Some(r),
            Item::Folder(f) => {
                if let Some(r) = first_request(&f.items) {
                    return Some(r);
                }
            }
        }
    }
    None
}

pub(super) fn flatten_requests(items: &[Item], out: &mut Vec<Request>) {
    for it in items {
        match it {
            Item::Request(r) => out.push(r.clone()),
            Item::Folder(f) => flatten_requests(&f.items, out),
        }
    }
}

pub(super) fn find_item_ref<'a>(items: &'a [Item], id: &str) -> Option<&'a Item> {
    for it in items {
        if it.id() == id {
            return Some(it);
        }
        if let Item::Folder(f) = it
            && let Some(x) = find_item_ref(&f.items, id)
        {
            return Some(x);
        }
    }
    None
}

pub(super) fn find_folder_ref<'a>(items: &'a [Item], id: &str) -> Option<&'a Folder> {
    match find_item_ref(items, id) {
        Some(Item::Folder(f)) => Some(f),
        _ => None,
    }
}

pub(super) fn list_folders(items: &[Item], prefix: &str, out: &mut Vec<(String, String)>) {
    for it in items {
        if let Item::Folder(f) = it {
            let name = if prefix.is_empty() { f.name.clone() } else { format!("{} / {}", prefix, f.name) };
            out.push((f.id.clone(), name.clone()));
            list_folders(&f.items, &name, out);
        }
    }
}

pub(super) fn items_match(items: &[Item], filter: &str) -> bool {
    items.iter().any(|it| match it {
        Item::Request(r) => r.name.to_lowercase().contains(filter) || r.url.to_lowercase().contains(filter),
        Item::Folder(f) => f.name.to_lowercase().contains(filter) || items_match(&f.items, filter),
    })
}

pub(super) fn pretty_path(p: &str) -> String {
    p.trim_start_matches("collections/")
        .trim_start_matches("environments/")
        .replace(workspace::REQUEST_EXT, "")
        .replace(workspace::ENV_EXT, " (environment)")
        .replace("/folder.json", " (folder)")
        .replace("/collection.json", " (collection)")
        .replace('/', " › ")
}

pub(super) fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(crate::ICON).ok()?.into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture("pigeon logo", pixels, egui::TextureOptions::LINEAR))
}
