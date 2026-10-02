// SPDX-License-Identifier: GPL-3.0-only

//! Native context-menu popups, with an in-application fallback.
//!
//! The popup only draws the labels it is given; what each entry means is
//! decided by whoever opened the menu.

#[cfg(target_os = "linux")]
pub mod render;
#[cfg(target_os = "linux")]
pub mod wayland;

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
