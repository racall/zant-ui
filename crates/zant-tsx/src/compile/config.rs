//! `app.config.json`: the config the compiler reads (title, bundle id, single instance, window).

/// The app configuration from `app.config.json` (the subset generated Rust
/// needs).
#[derive(Clone, Debug)]
pub struct ConfigSpec {
    pub title: String,
    pub bundle_id: String,
    pub single_instance: bool,
    pub window: WindowConfig,
}

impl Default for ConfigSpec {
    fn default() -> Self {
        ConfigSpec {
            title: "Zant App".to_owned(),
            bundle_id: "dev.zant.app".to_owned(),
            single_instance: false,
            window: WindowConfig::default(),
        }
    }
}

/// The window section of `app.config.json`, with its defaults.
#[derive(Clone, Debug)]
pub struct WindowConfig {
    pub style: String,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
    pub background: u32,
    pub resizable: bool,
    pub always_on_top: bool,
    pub movable_by_background: bool,
    pub center: bool,
    pub titlebar_height: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            style: "native".to_owned(),
            width: 560.0,
            height: 420.0,
            min_width: 480.0,
            min_height: 320.0,
            background: 0xfff5_f5f5,
            resizable: true,
            always_on_top: false,
            movable_by_background: false,
            center: true,
            titlebar_height: 0.0,
        }
    }
}

/// Reads `app.config.json` (the fields generated Rust uses; malformed values
/// are an error, unknown fields are ignored).
pub fn read_config(path: &str) -> Result<ConfigSpec, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let mut config = ConfigSpec::default();
    let Some(object) = value.as_object() else {
        return Err("not a JSON object".to_owned());
    };
    if let Some(title) = object.get("title").and_then(|value| value.as_str()) {
        config.title = title.to_owned();
    }
    if let Some(bundle) = object.get("bundle_id").and_then(|value| value.as_str()) {
        config.bundle_id = bundle.to_owned();
    }
    if let Some(flag) = object
        .get("single_instance")
        .and_then(|value| value.as_bool())
    {
        config.single_instance = flag;
    }
    if let Some(window) = object.get("window").and_then(|value| value.as_object()) {
        let window_config = &mut config.window;
        if let Some(style) = window.get("style").and_then(|value| value.as_str()) {
            window_config.style = style.to_owned();
        }
        if let Some(value) = window.get("width").and_then(|value| value.as_f64()) {
            window_config.width = value as f32;
        }
        if let Some(value) = window.get("height").and_then(|value| value.as_f64()) {
            window_config.height = value as f32;
        }
        if let Some(value) = window.get("min_width").and_then(|value| value.as_f64()) {
            window_config.min_width = value as f32;
        }
        if let Some(value) = window.get("min_height").and_then(|value| value.as_f64()) {
            window_config.min_height = value as f32;
        }
        if let Some(value) = window
            .get("titlebar_height")
            .and_then(|value| value.as_f64())
        {
            window_config.titlebar_height = value as f32;
        }
        if let Some(background) = window.get("background").and_then(|value| value.as_str()) {
            window_config.background = parse_color(background)?;
        }
        if let Some(flag) = window.get("resizable").and_then(|value| value.as_bool()) {
            window_config.resizable = flag;
        }
        if let Some(flag) = window
            .get("always_on_top")
            .and_then(|value| value.as_bool())
        {
            window_config.always_on_top = flag;
        }
        if let Some(flag) = window
            .get("movable_by_background")
            .and_then(|value| value.as_bool())
        {
            window_config.movable_by_background = flag;
        }
        if let Some(flag) = window.get("center").and_then(|value| value.as_bool()) {
            window_config.center = flag;
        }
    }
    Ok(config)
}

pub(super) fn parse_color(text: &str) -> Result<u32, String> {
    let value = text.strip_prefix('#').unwrap_or(text);
    match value.len() {
        6 => u32::from_str_radix(value, 16)
            .map(|rgb| 0xff00_0000 | rgb)
            .map_err(|_| format!("invalid color `{text}`")),
        8 => u32::from_str_radix(value, 16).map_err(|_| format!("invalid color `{text}`")),
        _ => Err(format!("invalid color `{text}`")),
    }
}
