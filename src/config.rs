use eframe::egui::{ecolor::Hsva, Color32};
use std::path::PathBuf;

// ---------- Темы ----------

#[derive(Clone, Copy)]
pub(crate) struct Theme {
    pub name: &'static str,
    pub bg_top: [u8; 3],
    pub bg_bottom: [u8; 3],
    pub blob_a: [u8; 3],
    pub blob_b: [u8; 3],
    pub accent: [u8; 3],
    pub num: [u8; 3],  // цифры
    pub func: [u8; 3], // C ± %
    pub text: [u8; 3],
    pub light: bool,
}

#[allow(clippy::too_many_arguments)]
const fn theme(
    name: &'static str,
    bg_top: [u8; 3],
    bg_bottom: [u8; 3],
    blob_a: [u8; 3],
    blob_b: [u8; 3],
    accent: [u8; 3],
    num: [u8; 3],
    func: [u8; 3],
    text: [u8; 3],
    light: bool,
) -> Theme {
    Theme {
        name,
        bg_top,
        bg_bottom,
        blob_a,
        blob_b,
        accent,
        num,
        func,
        text,
        light,
    }
}

pub(crate) const THEMES: [Theme; 9] = [
    theme("Графит", [48, 48, 54], [16, 16, 20], [255, 159, 10], [90, 90, 255], [255, 159, 10], [78, 78, 82], [104, 104, 108], [255, 255, 255], false),
    theme("Полночь", [26, 30, 70], [8, 8, 28], [99, 102, 241], [168, 85, 247], [124, 131, 255], [46, 50, 100], [72, 78, 134], [240, 242, 255], false),
    theme("Аврора", [10, 54, 60], [5, 20, 30], [45, 212, 191], [74, 222, 128], [45, 212, 191], [24, 72, 78], [38, 102, 106], [235, 255, 252], false),
    theme("Закат", [74, 26, 54], [24, 10, 32], [251, 113, 133], [251, 146, 60], [251, 113, 133], [96, 42, 74], [132, 62, 96], [255, 240, 244], false),
    theme("Лес", [22, 52, 36], [8, 22, 16], [163, 230, 53], [16, 185, 129], [163, 230, 53], [34, 72, 50], [52, 98, 70], [240, 255, 244], false),
    theme("Океан", [8, 50, 78], [3, 18, 38], [14, 165, 233], [99, 102, 241], [56, 189, 248], [18, 72, 108], [32, 102, 144], [236, 248, 255], false),
    theme("Неон", [24, 10, 42], [6, 4, 16], [236, 72, 153], [34, 211, 238], [34, 211, 238], [38, 26, 68], [60, 40, 104], [244, 238, 255], false),
    theme("Сакура", [255, 238, 245], [242, 214, 232], [255, 150, 190], [190, 160, 255], [236, 72, 153], [255, 255, 255], [250, 226, 238], [62, 30, 52], true),
    theme("Лёд", [234, 245, 255], [206, 225, 247], [96, 165, 250], [45, 212, 191], [37, 99, 235], [255, 255, 255], [219, 234, 254], [20, 32, 54], true),
];

// ---------- VPN (демо-данные) ----------

/// (название, цена, подпись)
pub(crate) const PLANS: [(&str, &str, &str); 3] = [
    ("Месяц", "199 руб./мес", "Гибкая подписка"),
    ("Год", "1 490 руб./год", "Выгода 38%"),
    ("Навсегда", "3 990 руб.", "Один платёж, без продлений"),
];

/// (код страны, город, пинг)
pub(crate) const SERVERS: [(&str, &str, u32); 5] = [
    ("FI", "Хельсинки", 18),
    ("DE", "Франкфурт", 32),
    ("NL", "Амстердам", 35),
    ("US", "Нью-Йорк", 120),
    ("JP", "Токио", 190),
];

// ---------- Настройки ----------

#[derive(Clone, PartialEq)]
pub(crate) struct Settings {
    pub name: String,
    pub theme: usize,
    pub custom_accent: bool,
    pub hue: f32,
    pub glass: f32,
    pub shape: u8, // 0 круглые, 1 мягкие, 2 квадратные
    pub animated_bg: bool,
    pub glow: bool,
    pub ripple: bool,
    pub always_on_top: bool,
    pub show_expr: bool,
    pub group_digits: bool,
    pub plan: u8, // 0 нет, 1..=3 — индекс тарифа + 1
    pub vpn_server: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            name: "Гость".to_string(),
            theme: 0,
            custom_accent: false,
            hue: 0.58,
            glass: 0.7,
            shape: 0,
            animated_bg: true,
            glow: true,
            ripple: true,
            always_on_top: false,
            show_expr: true,
            group_digits: false,
            plan: 0,
            vpn_server: 0,
        }
    }
}

impl Settings {
    pub fn theme(&self) -> Theme {
        THEMES[self.theme.min(THEMES.len() - 1)]
    }

    pub fn accent_color(&self) -> Color32 {
        if self.custom_accent {
            Hsva::new(self.hue, 0.72, 1.0, 1.0).into()
        } else {
            let a = self.theme().accent;
            Color32::from_rgb(a[0], a[1], a[2])
        }
    }

    /// Цвет текста на акцентной заливке: тёмный на светлых акцентах, белый на тёмных.
    pub fn on_accent(&self) -> Color32 {
        let a = self.accent_color();
        let lum = 0.299 * a.r() as f32 + 0.587 * a.g() as f32 + 0.114 * a.b() as f32;
        if lum > 185.0 {
            Color32::from_rgb(24, 24, 28)
        } else {
            Color32::WHITE
        }
    }

    /// Сбрасывает только внешний вид, имя и подписка остаются.
    pub fn reset_look(&mut self) {
        let (name, plan, server) = (self.name.clone(), self.plan, self.vpn_server);
        *self = Self::default();
        self.name = name;
        self.plan = plan;
        self.vpn_server = server;
    }

    pub fn load() -> Self {
        let mut s = Self::default();
        let Ok(text) = std::fs::read_to_string(config_dir().join("settings.cfg")) else {
            return s;
        };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            let on = v == "1";
            match k.trim() {
                "name" => s.name = v.chars().take(18).collect(),
                "theme" => {
                    if let Ok(n) = v.parse::<usize>() {
                        s.theme = n.min(THEMES.len() - 1);
                    }
                }
                "custom_accent" => s.custom_accent = on,
                "hue" => {
                    if let Ok(f) = v.parse::<f32>() {
                        s.hue = f.clamp(0.0, 1.0);
                    }
                }
                "glass" => {
                    if let Ok(f) = v.parse::<f32>() {
                        s.glass = f.clamp(0.35, 1.0);
                    }
                }
                "shape" => {
                    if let Ok(n) = v.parse::<u8>() {
                        s.shape = n.min(2);
                    }
                }
                "animated_bg" => s.animated_bg = on,
                "glow" => s.glow = on,
                "ripple" => s.ripple = on,
                "always_on_top" => s.always_on_top = on,
                "show_expr" => s.show_expr = on,
                "group_digits" => s.group_digits = on,
                "plan" => {
                    if let Ok(n) = v.parse::<u8>() {
                        s.plan = n.min(PLANS.len() as u8);
                    }
                }
                "vpn_server" => {
                    if let Ok(n) = v.parse::<usize>() {
                        s.vpn_server = n.min(SERVERS.len() - 1);
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let b = |v: bool| if v { "1" } else { "0" };
        let text = format!(
            "name={}\ntheme={}\ncustom_accent={}\nhue={}\nglass={}\nshape={}\nanimated_bg={}\nglow={}\nripple={}\nalways_on_top={}\nshow_expr={}\ngroup_digits={}\nplan={}\nvpn_server={}\n",
            self.name.replace('\n', " "),
            self.theme,
            b(self.custom_accent),
            self.hue,
            self.glass,
            self.shape,
            b(self.animated_bg),
            b(self.glow),
            b(self.ripple),
            b(self.always_on_top),
            b(self.show_expr),
            b(self.group_digits),
            self.plan,
            self.vpn_server,
        );
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("settings.cfg"), text);
    }
}

// ---------- Пути ----------

pub(crate) fn config_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wrong-calculator")
}

pub(crate) fn avatar_path() -> PathBuf {
    config_dir().join("avatar.png")
}
