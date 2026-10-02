// SPDX-License-Identifier: GPL-3.0-only

//! Native context-menu popups, with an in-application fallback.
//!
//! The popup only draws the labels it is given; what each entry means is
//! decided by whoever opened the menu.

#[cfg(target_os = "linux")]
pub mod render;
#[cfg(target_os = "linux")]
pub mod wayland;

/// The width of the menu.
pub const WIDTH: f32 = 190.0;
/// The height of a single menu entry.
pub const ROW_HEIGHT: f32 = 32.0;
/// The padding above the first and below the last entry.
pub const PADDING: f32 = 6.0;

/// The height a menu with `count` entries occupies.
pub fn height(count: usize) -> f32 {
    PADDING * 2.0 + ROW_HEIGHT * count as f32
}

/// Where the menu grows from the click, so that it stays visible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Downwards from the click, the usual case.
    Down,
    /// Upwards from the click, when there is no room below.
    Up,
}

/// The result of pumping native popup events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PopupEvent {
    /// Nothing happened.
    None,
    /// The user selected the action at the given index.
    Selected(usize),
    /// The compositor dismissed the popup.
    Dismissed,
}

/// Pumps native popup events.
pub fn pump() -> PopupEvent {
    #[cfg(target_os = "linux")]
    {
        wayland::pump()
    }

    #[cfg(not(target_os = "linux"))]
    {
        PopupEvent::None
    }
}

/// Hides any native popup.
pub fn hide() {
    #[cfg(target_os = "linux")]
    wayland::hide();
}
