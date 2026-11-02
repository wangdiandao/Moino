//! L2 localized interface copy and shared presentation, depending on the theme layer.
use gpui_kit::SharedString;
rust_i18n::i18n!("locales", fallback = "en");

/// Resolves one interface label from the application's effective language.
pub fn text(key: &str) -> SharedString {
    rust_i18n::t!(key).into_owned().into()
}

/// Resolves an editing failure without exposing backend exceptions or document text.
pub fn editing_error(code: &str) -> SharedString {
    text(match code {
        "model.invariant" => "editor.protected",
        "editing.unavailable" => "editor.unavailable",
        "document.stale" | "document.stale_transaction" => "editor.stale",
        "document.invalid_transaction" => "editor.invalid",
        "document.revision_exhausted" => "editor.exhausted",
        _ => "editor.failed",
    })
}

/// Applies the product's supported-language mapping.
pub fn set_language(locale: &str) {
    rust_i18n::set_locale(if locale.to_ascii_lowercase().starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    });
}
