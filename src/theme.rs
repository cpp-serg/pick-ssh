use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub bg_hl: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub fg_bright: Color,
    pub accent_filter: Color,
    pub accent_marker: Color,
    pub accent_header: Color,
    pub accent_info: Color,
}

pub const GRUVBOX_DARK: Theme = Theme {
    bg: Color::Rgb(29, 32, 33),
    bg_hl: Color::Rgb(80, 73, 69),
    fg: Color::Rgb(235, 219, 178),
    fg_dim: Color::Rgb(168, 153, 132),
    fg_bright: Color::Rgb(251, 241, 199),
    accent_filter: Color::Rgb(250, 189, 47),
    accent_marker: Color::Rgb(142, 192, 124),
    accent_header: Color::Rgb(254, 128, 25),
    accent_info: Color::Rgb(131, 165, 152),
};

pub const GRUVBOX_LIGHT: Theme = Theme {
    bg: Color::Rgb(251, 241, 199),
    bg_hl: Color::Rgb(213, 196, 161),
    fg: Color::Rgb(60, 56, 54),
    fg_dim: Color::Rgb(124, 111, 100),
    fg_bright: Color::Rgb(40, 40, 40),
    accent_filter: Color::Rgb(181, 118, 20),
    accent_marker: Color::Rgb(121, 116, 14),
    accent_header: Color::Rgb(175, 58, 3),
    accent_info: Color::Rgb(7, 102, 120),
};

pub const DRACULA: Theme = Theme {
    bg: Color::Rgb(40, 42, 54),
    bg_hl: Color::Rgb(68, 71, 90),
    fg: Color::Rgb(248, 248, 242),
    fg_dim: Color::Rgb(98, 114, 164),
    fg_bright: Color::Rgb(255, 255, 255),
    accent_filter: Color::Rgb(241, 250, 140),
    accent_marker: Color::Rgb(80, 250, 123),
    accent_header: Color::Rgb(255, 121, 198),
    accent_info: Color::Rgb(139, 233, 253),
};

pub const NORD: Theme = Theme {
    bg: Color::Rgb(46, 52, 64),
    bg_hl: Color::Rgb(67, 76, 94),
    fg: Color::Rgb(216, 222, 233),
    fg_dim: Color::Rgb(76, 86, 106),
    fg_bright: Color::Rgb(236, 239, 244),
    accent_filter: Color::Rgb(235, 203, 139),
    accent_marker: Color::Rgb(163, 190, 140),
    accent_header: Color::Rgb(208, 135, 112),
    accent_info: Color::Rgb(136, 192, 208),
};

pub const CATPPUCCIN_MOCHA: Theme = Theme {
    bg: Color::Rgb(30, 30, 46),
    bg_hl: Color::Rgb(69, 71, 90),
    fg: Color::Rgb(205, 214, 244),
    fg_dim: Color::Rgb(127, 132, 156),
    fg_bright: Color::Rgb(255, 255, 255),
    accent_filter: Color::Rgb(249, 226, 175),
    accent_marker: Color::Rgb(166, 227, 161),
    accent_header: Color::Rgb(250, 179, 135),
    accent_info: Color::Rgb(137, 180, 250),
};

pub const CATPPUCCIN_LATTE: Theme = Theme {
    bg: Color::Rgb(239, 241, 245),
    bg_hl: Color::Rgb(204, 208, 218),
    fg: Color::Rgb(76, 79, 105),
    fg_dim: Color::Rgb(140, 143, 161),
    fg_bright: Color::Rgb(32, 32, 32),
    accent_filter: Color::Rgb(223, 142, 29),
    accent_marker: Color::Rgb(64, 160, 43),
    accent_header: Color::Rgb(254, 100, 11),
    accent_info: Color::Rgb(30, 102, 245),
};

pub const SOLARIZED_DARK: Theme = Theme {
    bg: Color::Rgb(0, 43, 54),
    bg_hl: Color::Rgb(7, 54, 66),
    fg: Color::Rgb(131, 148, 150),
    fg_dim: Color::Rgb(88, 110, 117),
    fg_bright: Color::Rgb(238, 232, 213),
    accent_filter: Color::Rgb(181, 137, 0),
    accent_marker: Color::Rgb(133, 153, 0),
    accent_header: Color::Rgb(203, 75, 22),
    accent_info: Color::Rgb(38, 139, 210),
};

pub const SOLARIZED_LIGHT: Theme = Theme {
    bg: Color::Rgb(253, 246, 227),
    bg_hl: Color::Rgb(238, 232, 213),
    fg: Color::Rgb(101, 123, 131),
    fg_dim: Color::Rgb(147, 161, 161),
    fg_bright: Color::Rgb(7, 54, 66),
    accent_filter: Color::Rgb(181, 137, 0),
    accent_marker: Color::Rgb(133, 153, 0),
    accent_header: Color::Rgb(203, 75, 22),
    accent_info: Color::Rgb(38, 139, 210),
};

pub const TOKYONIGHT: Theme = Theme {
    bg: Color::Rgb(26, 27, 38),
    bg_hl: Color::Rgb(41, 46, 66),
    fg: Color::Rgb(169, 177, 214),
    fg_dim: Color::Rgb(86, 95, 137),
    fg_bright: Color::Rgb(195, 202, 231),
    accent_filter: Color::Rgb(224, 175, 104),
    accent_marker: Color::Rgb(158, 206, 106),
    accent_header: Color::Rgb(255, 158, 100),
    accent_info: Color::Rgb(125, 174, 163),
};

pub const TOKYONIGHT_STORM: Theme = Theme {
    bg: Color::Rgb(36, 40, 59),
    bg_hl: Color::Rgb(52, 56, 78),
    fg: Color::Rgb(169, 177, 214),
    fg_dim: Color::Rgb(86, 95, 137),
    fg_bright: Color::Rgb(195, 202, 231),
    accent_filter: Color::Rgb(224, 175, 104),
    accent_marker: Color::Rgb(158, 206, 106),
    accent_header: Color::Rgb(255, 158, 100),
    accent_info: Color::Rgb(125, 174, 163),
};

pub const ONEDARK: Theme = Theme {
    bg: Color::Rgb(40, 44, 52),
    bg_hl: Color::Rgb(62, 68, 81),
    fg: Color::Rgb(171, 178, 191),
    fg_dim: Color::Rgb(92, 99, 112),
    fg_bright: Color::Rgb(220, 223, 228),
    accent_filter: Color::Rgb(229, 192, 123),
    accent_marker: Color::Rgb(152, 195, 121),
    accent_header: Color::Rgb(209, 154, 102),
    accent_info: Color::Rgb(97, 175, 239),
};

pub const MONOKAI: Theme = Theme {
    bg: Color::Rgb(39, 40, 34),
    bg_hl: Color::Rgb(73, 72, 62),
    fg: Color::Rgb(248, 248, 242),
    fg_dim: Color::Rgb(117, 113, 94),
    fg_bright: Color::Rgb(255, 255, 255),
    accent_filter: Color::Rgb(230, 219, 116),
    accent_marker: Color::Rgb(166, 226, 46),
    accent_header: Color::Rgb(253, 151, 31),
    accent_info: Color::Rgb(102, 217, 239),
};

pub const ZENBURN: Theme = Theme {
    bg: Color::Rgb(63, 63, 63),
    bg_hl: Color::Rgb(79, 79, 79),
    fg: Color::Rgb(220, 220, 204),
    fg_dim: Color::Rgb(127, 159, 127),
    fg_bright: Color::Rgb(255, 255, 255),
    accent_filter: Color::Rgb(240, 223, 175),
    accent_marker: Color::Rgb(127, 159, 127),
    accent_header: Color::Rgb(223, 175, 143),
    accent_info: Color::Rgb(140, 208, 211),
};

pub const BASE16_DEFAULT_DARK: Theme = Theme {
    bg: Color::Rgb(24, 24, 24),
    bg_hl: Color::Rgb(56, 56, 56),
    fg: Color::Rgb(216, 216, 216),
    fg_dim: Color::Rgb(88, 88, 88),
    fg_bright: Color::Rgb(248, 248, 248),
    accent_filter: Color::Rgb(247, 202, 136),
    accent_marker: Color::Rgb(168, 219, 140),
    accent_header: Color::Rgb(220, 150, 86),
    accent_info: Color::Rgb(124, 175, 194),
};

pub const ROSE_PINE: Theme = Theme {
    bg: Color::Rgb(25, 23, 36),
    bg_hl: Color::Rgb(38, 35, 53),
    fg: Color::Rgb(224, 222, 244),
    fg_dim: Color::Rgb(110, 106, 134),
    fg_bright: Color::Rgb(224, 222, 244),
    accent_filter: Color::Rgb(246, 193, 119),
    accent_marker: Color::Rgb(49, 116, 143),
    accent_header: Color::Rgb(235, 111, 146),
    accent_info: Color::Rgb(156, 207, 216),
};

pub const ROSE_PINE_MOON: Theme = Theme {
    bg: Color::Rgb(35, 33, 54),
    bg_hl: Color::Rgb(57, 53, 82),
    fg: Color::Rgb(224, 222, 244),
    fg_dim: Color::Rgb(110, 106, 134),
    fg_bright: Color::Rgb(224, 222, 244),
    accent_filter: Color::Rgb(246, 193, 119),
    accent_marker: Color::Rgb(62, 143, 176),
    accent_header: Color::Rgb(235, 111, 146),
    accent_info: Color::Rgb(156, 207, 216),
};

pub const ROSE_PINE_DAWN: Theme = Theme {
    bg: Color::Rgb(250, 244, 237),
    bg_hl: Color::Rgb(242, 233, 222),
    fg: Color::Rgb(87, 82, 121),
    fg_dim: Color::Rgb(152, 147, 165),
    fg_bright: Color::Rgb(87, 82, 121),
    accent_filter: Color::Rgb(234, 157, 52),
    accent_marker: Color::Rgb(40, 105, 131),
    accent_header: Color::Rgb(180, 99, 122),
    accent_info: Color::Rgb(86, 148, 159),
};

pub const KANAGAWA: Theme = Theme {
    bg: Color::Rgb(31, 31, 40),
    bg_hl: Color::Rgb(54, 54, 70),
    fg: Color::Rgb(220, 215, 186),
    fg_dim: Color::Rgb(114, 113, 105),
    fg_bright: Color::Rgb(240, 235, 216),
    accent_filter: Color::Rgb(228, 104, 118),
    accent_marker: Color::Rgb(152, 187, 108),
    accent_header: Color::Rgb(255, 160, 102),
    accent_info: Color::Rgb(126, 156, 216),
};

pub const KANAGAWA_DRAGON: Theme = Theme {
    bg: Color::Rgb(24, 21, 26),
    bg_hl: Color::Rgb(43, 40, 47),
    fg: Color::Rgb(192, 163, 134),
    fg_dim: Color::Rgb(113, 105, 95),
    fg_bright: Color::Rgb(220, 200, 170),
    accent_filter: Color::Rgb(228, 104, 118),
    accent_marker: Color::Rgb(135, 169, 97),
    accent_header: Color::Rgb(231, 138, 78),
    accent_info: Color::Rgb(138, 167, 231),
};

pub const KANAGAWA_LOTUS: Theme = Theme {
    bg: Color::Rgb(245, 240, 227),
    bg_hl: Color::Rgb(228, 218, 199),
    fg: Color::Rgb(84, 84, 82),
    fg_dim: Color::Rgb(143, 143, 128),
    fg_bright: Color::Rgb(29, 29, 28),
    accent_filter: Color::Rgb(214, 75, 99),
    accent_marker: Color::Rgb(110, 150, 75),
    accent_header: Color::Rgb(204, 107, 44),
    accent_info: Color::Rgb(99, 123, 184),
};

pub const THEMES: &[(&str, Theme)] = &[
    ("gruvbox-dark", GRUVBOX_DARK),
    ("gruvbox-light", GRUVBOX_LIGHT),
    ("dracula", DRACULA),
    ("nord", NORD),
    ("catppuccin-mocha", CATPPUCCIN_MOCHA),
    ("catppuccin-latte", CATPPUCCIN_LATTE),
    ("solarized-dark", SOLARIZED_DARK),
    ("solarized-light", SOLARIZED_LIGHT),
    ("tokyonight", TOKYONIGHT),
    ("tokyonight-storm", TOKYONIGHT_STORM),
    ("onedark", ONEDARK),
    ("monokai", MONOKAI),
    ("zenburn", ZENBURN),
    ("base16-default-dark", BASE16_DEFAULT_DARK),
    ("rose-pine", ROSE_PINE),
    ("rose-pine-moon", ROSE_PINE_MOON),
    ("rose-pine-dawn", ROSE_PINE_DAWN),
    ("kanagawa", KANAGAWA),
    ("kanagawa-dragon", KANAGAWA_DRAGON),
    ("kanagawa-lotus", KANAGAWA_LOTUS),
];

pub const THEME_NAMES: &[&str] = &[
    "gruvbox-dark",
    "gruvbox-light",
    "dracula",
    "nord",
    "catppuccin-mocha",
    "catppuccin-latte",
    "solarized-dark",
    "solarized-light",
    "tokyonight",
    "tokyonight-storm",
    "onedark",
    "monokai",
    "zenburn",
    "base16-default-dark",
    "rose-pine",
    "rose-pine-moon",
    "rose-pine-dawn",
    "kanagawa",
    "kanagawa-dragon",
    "kanagawa-lotus",
];

pub fn theme_by_index(index: usize) -> Theme {
    THEMES[index % THEMES.len()].1
}

pub fn theme_name_by_index(index: usize) -> &'static str {
    THEMES[index % THEMES.len()].0
}

/// Parse `PICK_SSH_CONFIG` env var for `theme=` key.
/// Format: `PICK_SSH_CONFIG="color=auto,theme=gruvbox"`
pub fn theme_from_env() -> Option<String> {
    let val = std::env::var("PICK_SSH_CONFIG").ok()?;
    for part in val.split(',') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("theme=") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Resolve theme: CLI flag > config file > env var > default (gruvbox-dark).
/// Warns on stderr for unknown theme names.
/// Returns (Theme, index into THEMES).
pub fn resolve_theme(cli_theme: Option<&str>, config_theme: Option<&str>) -> (Theme, usize) {
    let name = cli_theme
        .map(String::from)
        .or_else(|| config_theme.map(String::from))
        .or_else(theme_from_env);

    match name {
        Some(n) => {
            // Resolve alias to canonical name
            let canonical = match n.as_str() {
                "gruvbox" => "gruvbox-dark",
                "catppuccin" => "catppuccin-mocha",
                "solarized" => "solarized-dark",
                "base16" => "base16-default-dark",
                "rose-pine" | "rosepine" => "rose-pine",
                other => other,
            };
            match THEMES.iter().position(|&(name, _)| name == canonical) {
                Some(idx) => (THEMES[idx].1, idx),
                None => {
                    eprintln!("warning: unknown theme '{n}', falling back to gruvbox-dark");
                    (GRUVBOX_DARK, 0)
                }
            }
        }
        None => (GRUVBOX_DARK, 0),
    }
}
