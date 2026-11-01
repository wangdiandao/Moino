//! L1 built-in Moino palettes, bundled fonts and projection to GPUI Kit.
use gpui_kit::base::text::TextViewStyle;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, WindowAppearance, rgb};
use gpui_kit::{prelude::*, *};
use std::borrow::Cow;

/// Default content-box maximum width, including its horizontal padding.
pub fn content_width() -> Rems {
    rems(860. / 13.)
}
/// Default content-box horizontal inset.
pub fn content_inset() -> Rems {
    rems(30. / 13.)
}
/// Body size relative to the application interface font.
pub fn body_size() -> Rems {
    rems(16. / 13.)
}
/// Code size relative to the application interface font.
pub fn code_size() -> Rems {
    rems(14. / 13.)
}

/// Builds rich-text presentation from the effective built-in theme.
pub fn text_style(cx: &App) -> TextViewStyle {
    use gpui_kit::component::ActiveTheme;
    let theme = cx
        .try_global::<ThemeState>()
        .map_or(BuiltinTheme::Paper, |state| state.theme);
    let heading = theme.color(0x17201C, 0xE2E9E4);
    TextViewStyle::default()
        .with_foreground(cx.theme().foreground)
        .with_muted_foreground(cx.theme().muted_foreground)
        .with_link(cx.theme().link)
        .with_selection(cx.theme().selection)
        .with_border(cx.theme().border)
        .with_code_background(theme.code_background())
        .with_dark(theme == BuiltinTheme::Inkstone)
        .with_heading(move |level| {
            let font = [36., 28., 24., 20., 16., 16.][usize::from(level.clamp(1, 6) - 1)];
            StyleRefinement::default()
                .text_size(rems(font / 13.))
                .line_height(relative(1.3))
                .text_color(heading)
        })
}

/// Built-in members of the Moino theme family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinTheme {
    /// Warm paper with dark green ink.
    Paper,
    /// Dark inkstone with jade accents.
    Inkstone,
}

/// Appearance policy, independent from the currently effective palette.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppearanceMode {
    /// Follow operating-system changes.
    #[default]
    System,
    /// Keep the light palette.
    Light,
    /// Keep the dark palette.
    Dark,
}
impl BuiltinTheme {
    /// Stable theme identifier for configuration.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Paper => "moino-paper",
            Self::Inkstone => "moino-inkstone",
        }
    }
    /// Resolves the system appearance without altering a document.
    pub fn from_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Inkstone,
            _ => Self::Paper,
        }
    }
    fn color(self, paper: u32, inkstone: u32) -> Hsla {
        rgb(match self {
            Self::Paper => paper,
            Self::Inkstone => inkstone,
        })
        .into()
    }
    /// Foreground for expanded source delimiters.
    pub fn marker(self) -> Hsla {
        self.color(0x9AA69F, 0x5E6D65)
    }
    /// Flat code surface background.
    pub fn code_background(self) -> Hsla {
        self.color(0xEEECE5, 0x17201B)
    }
}

/// Current product appearance and revision, independent of document revisions.
pub struct ThemeState {
    theme: BuiltinTheme,
    revision: u64,
    mode: AppearanceMode,
}
impl Global for ThemeState {}
impl ThemeState {
    /// Current system-following or fixed appearance policy.
    pub const fn mode(&self) -> AppearanceMode {
        self.mode
    }
    /// Effective built-in theme.
    pub const fn theme(&self) -> BuiltinTheme {
        self.theme
    }
    /// Invalidation identity for theme-dependent derivation.
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

/// Registers the four text faces of both product font families.
pub fn register_fonts(cx: &App) -> gpui_kit::Result<()> {
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!("../../../assets/fonts/inter-regular.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/fonts/inter-italic.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/fonts/inter-bold.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/fonts/inter-bolditalic.ttf")),
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/jetbrains-mono-regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/jetbrains-mono-italic.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/jetbrains-mono-bold.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../../../assets/fonts/jetbrains-mono-bolditalic.ttf"
        )),
    ])
}

/// Projects the built-in palette through Kit's synchronized theme update seam.
pub fn apply(theme: BuiltinTheme, cx: &mut App) {
    let mode = cx
        .try_global::<ThemeState>()
        .map_or(AppearanceMode::System, |state| state.mode);
    let revision = cx
        .try_global::<ThemeState>()
        .map_or(0, |state| state.revision);
    if cx
        .try_global::<ThemeState>()
        .is_some_and(|state| state.theme == theme)
    {
        return;
    }
    Theme::change(
        match theme {
            BuiltinTheme::Paper => ThemeMode::Light,
            BuiltinTheme::Inkstone => ThemeMode::Dark,
        },
        None,
        cx,
    );
    Theme::update(cx, |kit| {
        let c = &mut kit.colors;
        let color = |paper, inkstone| theme.color(paper, inkstone);
        c.background = color(0xF7F6F1, 0x111714);
        c.foreground = color(0x1E2622, 0xD3DCD6);
        c.border = color(0xDDDBD2, 0x25302A);
        c.input = c.border;
        c.muted = color(0xE8E6DE, 0x1B2420);
        c.muted_foreground = color(0x5B6660, 0x93A299);
        c.primary = color(0x2D6A4F, 0x5DB38A);
        c.primary_hover = color(0x24573F, 0x6FC49A);
        c.primary_active = color(0xC3CDC1, 0x284135);
        c.primary_foreground = color(0xFFFFFF, 0x0D1210);
        c.accent = color(0xE3E8E1, 0x1C2A23);
        c.accent_foreground = c.foreground;
        c.popover = color(0xFDFCF9, 0x18201C);
        c.popover_foreground = c.foreground;
        c.group_box = c.background;
        c.group_box_foreground = c.foreground;
        c.list = c.background;
        c.list_active = c.accent;
        c.list_active_border = c.primary;
        c.list_hover = c.muted;
        c.sidebar = color(0xEFEDE6, 0x0D1210);
        c.sidebar_foreground = c.foreground;
        c.sidebar_border = c.border;
        c.title_bar = c.sidebar;
        c.title_bar_border = c.border;
        c.status_bar = c.sidebar;
        c.status_bar_border = c.border;
        c.link = color(0x1F6B4A, 0x7DC6A0);
        c.link_hover = color(0x17543A, 0x9AD6B6);
        c.selection = color(0xD2E3D8, 0x29423A);
        c.caret = c.foreground;
        c.danger = color(0xB42318, 0xF08A7E);
        c.danger_foreground = c.primary_foreground;
        c.warning = color(0x8A5200, 0xF3C16A);
        c.success = color(0x1F6F43, 0x8DD6AA);
        c.info = color(0x34548A, 0x8FB4E8);
        c.ring = color(0x2D6A4F, 0x6FC49A);
        c.scrollbar_thumb = color(0xC9CCC4, 0x2F3A34);
        c.scrollbar_thumb_hover = color(0xB3B7AE, 0x3D4A43);
        kit.font_family = "Inter".into();
        kit.mono_font_family = "JetBrains Mono".into();
        kit.font_size = px(13.);
        kit.mono_font_size = px(14.);
        use gpui_kit::component::highlighter::{
            FontStyle as SyntaxFontStyle, FontWeightContent, SyntaxColors, ThemeStyle,
        };
        let style = |paper, inkstone| {
            Some(ThemeStyle {
                color: Some(color(paper, inkstone)),
                font_style: None,
                font_weight: None,
            })
        };
        let mut syntax = SyntaxColors::default();
        syntax.variable = style(0x1E2622, 0xD3DCD6);
        syntax.embedded = syntax.variable;
        syntax.keyword = style(0x2D6A4F, 0x6FC49A);
        syntax.tag = syntax.keyword;
        syntax.punctuation_list_marker = syntax.keyword;
        syntax.string = style(0xA8432A, 0xE08D72);
        syntax.text_literal = syntax.string;
        syntax.text_code_span = syntax.string;
        syntax.constant = style(0x6A4C93, 0xB79BE0);
        syntax.boolean = syntax.constant;
        syntax.string_escape = syntax.constant;
        syntax.string_special = syntax.constant;
        syntax.preproc = syntax.constant;
        syntax.punctuation_special = syntax.constant;
        syntax.variable_special = syntax.constant;
        syntax.string_regex = style(0x8A5200, 0xE3B866);
        syntax.number = style(0x34548A, 0x8FB4E8);
        syntax.function = style(0x1F6470, 0x6CC0C9);
        syntax.constructor = syntax.function;
        syntax.type_ = style(0x7A5A1E, 0xD9B26A);
        syntax.attribute = syntax.type_;
        syntax.label = syntax.type_;
        syntax.enum_ = syntax.type_;
        syntax.variant = syntax.type_;
        syntax.property = style(0x3D5A50, 0xA9C4B6);
        syntax.operator = style(0x4F5A54, 0x93A299);
        syntax.punctuation = style(0x5B6660, 0x93A299);
        syntax.punctuation_bracket = syntax.punctuation;
        syntax.punctuation_delimiter = syntax.punctuation;
        syntax.comment = Some(ThemeStyle {
            font_style: Some(SyntaxFontStyle::Italic),
            ..style(0x616B65, 0x7E8C84).expect("built-in syntax")
        });
        syntax.comment_doc = syntax.comment;
        syntax.hint = syntax.comment;
        syntax.predictive = Some(ThemeStyle {
            color: Some(theme.marker()),
            font_style: Some(SyntaxFontStyle::Italic),
            font_weight: None,
        });
        syntax.title = Some(ThemeStyle {
            font_weight: Some(FontWeightContent::Bold),
            ..style(0x17201C, 0xE2E9E4).expect("built-in heading")
        });
        syntax.emphasis = Some(ThemeStyle {
            font_style: Some(SyntaxFontStyle::Italic),
            ..syntax.variable.expect("built-in foreground")
        });
        syntax.emphasis_strong = Some(ThemeStyle {
            font_weight: Some(FontWeightContent::Bold),
            ..syntax.variable.expect("built-in foreground")
        });
        syntax.link_text = style(0x1F6B4A, 0x7DC6A0);
        syntax.link_uri = Some(ThemeStyle {
            font_style: Some(SyntaxFontStyle::Underline),
            ..style(0x616B65, 0x7E8C84).expect("built-in URI")
        });
        let mut highlight = (*kit.highlight_theme).clone();
        highlight.name = theme.id().into();
        highlight.appearance = kit.mode;
        highlight.style.editor_background = Some(c.background);
        highlight.style.editor_foreground = Some(c.foreground);
        highlight.style.syntax = syntax;
        kit.highlight_theme = std::sync::Arc::new(highlight);
    });
    cx.set_global(ThemeState {
        theme,
        revision: revision.checked_add(1).expect("theme revision exhausted"),
        mode,
    });
}

/// Changes the appearance policy for this application session.
pub fn set_mode(mode: AppearanceMode, cx: &mut App) {
    let theme = match mode {
        AppearanceMode::System => BuiltinTheme::from_appearance(cx.window_appearance()),
        AppearanceMode::Light => BuiltinTheme::Paper,
        AppearanceMode::Dark => BuiltinTheme::Inkstone,
    };
    apply(theme, cx);
    cx.global_mut::<ThemeState>().mode = mode;
}

/// Reprojects a system appearance change only while following the system.
pub fn sync_system(appearance: WindowAppearance, cx: &mut App) {
    if cx
        .try_global::<ThemeState>()
        .is_none_or(|state| state.mode == AppearanceMode::System)
    {
        apply(BuiltinTheme::from_appearance(appearance), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{BuiltinTheme, ThemeState, apply, register_fonts};
    use gpui_kit::component::{ActiveTheme, Theme};
    use gpui_kit::{TestAppContext, rgb};

    #[gpui_kit::test]
    // Spec: docs/contracts/theme.md#投影到-gpui-kit
    fn palettes_synchronize_component_and_base_tokens(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            register_fonts(cx).expect("bundled fonts");
            for (ix, theme) in [BuiltinTheme::Paper, BuiltinTheme::Inkstone]
                .into_iter()
                .enumerate()
            {
                apply(theme, cx);
                let kit = Theme::global(cx);
                let expected = rgb(if theme == BuiltinTheme::Paper {
                    0xF7F6F1
                } else {
                    0x111714
                });
                assert_eq!(kit.background, expected.into());
                assert_eq!(
                    kit.primary,
                    gpui_kit::base::Theme::global(cx).tokens.colors.primary
                );
                assert_eq!(
                    kit.selection,
                    gpui_kit::base::Theme::global(cx).tokens.colors.selection
                );
                assert_eq!(
                    cx.global::<ThemeState>().revision(),
                    u64::try_from(ix + 1).expect("revision")
                );
                assert_eq!(cx.theme().font_family.as_ref(), "Inter");
            }
            apply(BuiltinTheme::Inkstone, cx);
            assert_eq!(cx.global::<ThemeState>().revision(), 2);
            super::set_mode(super::AppearanceMode::Dark, cx);
            super::sync_system(gpui_kit::WindowAppearance::Light, cx);
            assert_eq!(cx.global::<ThemeState>().theme(), BuiltinTheme::Inkstone);
        });
    }
}
