//! L2 localized interface copy and shared presentation, depending on the theme layer.
use gpui_kit::SharedString;
rust_i18n::i18n!("locales", fallback = "en");

/// Resolves one interface label from the application's effective language.
pub fn text(key: &str) -> SharedString {
    rust_i18n::t!(key).into_owned().into()
}

/// Applies the product's supported-language mapping.
pub fn set_language(locale: &str) {
    rust_i18n::set_locale(if locale.to_ascii_lowercase().starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    });
}
