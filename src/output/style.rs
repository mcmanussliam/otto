use std::sync::atomic::{AtomicBool, Ordering};

static COLORS_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn configure(no_color: bool) {
    let mut enabled = !no_color;

    if std::env::var_os("NO_COLOR").is_some() {
        enabled = false;
    }

    if let Ok(term) = std::env::var("TERM")
        && term.eq_ignore_ascii_case("dumb")
    {
        enabled = false;
    }

    if std::env::var("CLICOLOR_FORCE").ok().as_deref() == Some("1") {
        enabled = true;
    }

    COLORS_ENABLED.store(enabled, Ordering::Relaxed);
}

fn style(code: &str, text: &str) -> String {
    if text.is_empty() || !COLORS_ENABLED.load(Ordering::Relaxed) {
        return text.to_string();
    }

    format!("\x1b[{code}m{text}\x1b[0m")
}

pub fn bold(text: &str) -> String {
    style("1", text)
}

pub fn muted(text: &str) -> String {
    style("2", text)
}

pub fn accent(text: &str) -> String {
    style("36", text)
}

pub fn success(text: &str) -> String {
    style("32", text)
}

pub fn failure(text: &str) -> String {
    style("31", text)
}

pub fn warning(text: &str) -> String {
    style("33", text)
}

pub fn info(text: &str) -> String {
    style("96", text)
}

pub fn command(text: &str) -> String {
    style("96", text)
}

pub fn number(text: &str) -> String {
    style("96", text)
}

pub fn bullet(text: &str) -> String {
    style("94", text)
}
