// SPDX-License-Identifier: GPL-3.0-only

//! Spacing constants and shared styles.

use iced::Padding;

/// Extra-extra-extra small spacing.
pub const SPACE_XXXS: f32 = 2.0;
/// Extra-extra small spacing.
pub const SPACE_XXS: f32 = 4.0;
/// Extra small spacing.
pub const SPACE_XS: f32 = 8.0;
/// Small spacing.
pub const SPACE_S: f32 = 12.0;
/// Medium spacing.
pub const SPACE_M: f32 = 16.0;
/// Large spacing.
pub const SPACE_L: f32 = 24.0;
/// Extra large spacing.
pub const SPACE_XL: f32 = 32.0;
/// Extra-extra large spacing.
pub const SPACE_XXL: f32 = 48.0;

/// The padding applied to the edges of a page.
pub const PAGE_PADDING: Padding = Padding {
    top: 0.0,
    right: SPACE_XL,
    bottom: SPACE_S,
    left: SPACE_XL,
};

/// The background color of a card.
pub fn card_background(theme: &iced::Theme) -> iced::Color {
    theme.palette().background.weak.color
}
