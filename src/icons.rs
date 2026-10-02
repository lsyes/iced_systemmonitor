// SPDX-License-Identifier: GPL-3.0-only

//! Icon lookups through the freedesktop icon theme specification.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

use iced::{
    Element, Length, Widget,
    widget::{Space, image, svg},
};

/// A reference to an icon, either by icon theme name or by path.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Icon {
    /// An icon looked up by name in the icon theme.
    Name(String),
    /// An icon at a path, absolute or relative to the icon theme.
    Path(PathBuf),
}

impl Icon {
    pub fn name(name: impl Into<String>) -> Self {
        Self::Name(name.into())
    }

    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    /// Resolves this icon to a file path of the given size, if it exists.
    pub fn resolve(&self, size: u16) -> Option<PathBuf> {
        match self {
            Self::Path(path) if path.is_absolute() => path
                .exists()
                .then(|| path.clone())
                .or_else(|| fallback(size)),
            Self::Path(path) => lookup(&path.to_string_lossy(), size),
            Self::Name(name) => lookup(name, size),
        }
    }
}

static CACHE: LazyLock<Mutex<HashMap<(String, u16), Option<PathBuf>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The names tried, in order, when the requested icon is missing from the
/// theme, so that every entry can draw something meaningful.
const FALLBACKS: [&str; 3] = [
    "application-x-executable",
    "application-default-icon",
    "system-run",
];

/// Looks up an icon in the icon theme, with a small process-wide cache so that
/// rendering a list of icons does not hit the disk on every frame.
///
/// Falls back to a generic executable icon when the theme has no icon under
/// the requested name.
pub fn lookup(name: &str, size: u16) -> Option<PathBuf> {
    lookup_exact(name, size).or_else(|| fallback(size))
}

/// The first generic icon the theme provides.
fn fallback(size: u16) -> Option<PathBuf> {
    FALLBACKS.iter().find_map(|name| lookup_exact(name, size))
}

/// Looks up one name in the icon theme, without any fallback.
fn lookup_exact(name: &str, size: u16) -> Option<PathBuf> {
    let key = (name.to_string(), size);

    if let Some(hit) = CACHE.lock().unwrap().get(&key) {
        return hit.clone();
    }

    let path = freedesktop_icons::lookup(name)
        .with_size(size)
        .with_cache()
        .find();

    CACHE.lock().unwrap().insert(key, path.clone());

    path
}

/// Turns an [`Icon`] into a widget rendered at the given size.
///
/// Returns an empty spacer when the icon cannot be found.
pub fn view<'a, Message: 'a>(icon: &Icon, size: u16) -> Element<'a, Message> {
    let Some(path) = icon.resolve(size) else {
        let spacer = f32::from(size);
        return Space::new().width(spacer).height(spacer).boxed();
    };

    let is_svg = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));

    let icon_size = Length::Fixed(f32::from(size));

    if is_svg {
        svg(svg::Handle::from_path(path))
            .width(icon_size)
            .height(icon_size)
            .boxed()
    } else {
        image(image::Handle::from_path(path))
            .width(icon_size)
            .height(icon_size)
            .boxed()
    }
}
