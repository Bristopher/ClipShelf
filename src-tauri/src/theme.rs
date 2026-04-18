use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeTokens {
    pub title_bar: String,
    pub app_bg: String,
    pub panel_bg: String,
    pub text: String,
    pub text_muted: String,
    pub border: String,
    pub hover_bg: String,
    pub g1_accent: String,
    pub g2_accent: String,
    pub g3_accent: String,
    pub g4_accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub builtin: bool,
    pub tokens: ThemeTokens,
}

/// JSON export envelope with schema versioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeExport {
    pub schema: String,
    pub name: String,
    pub tokens: ThemeTokens,
}

pub const THEME_SCHEMA: &str = "gkey-theme-v1";

pub fn builtin_themes() -> Vec<Theme> {
    vec![
        Theme {
            id: "dark".into(),
            name: "Dark".into(),
            builtin: true,
            tokens: ThemeTokens {
                title_bar: "oklch(0.269 0 0)".into(),
                app_bg: "oklch(0.145 0 0)".into(),
                panel_bg: "oklch(0.205 0 0)".into(),
                text: "oklch(0.985 0 0)".into(),
                text_muted: "oklch(0.708 0 0)".into(),
                border: "oklch(1 0 0 / 10%)".into(),
                hover_bg: "oklch(1 0 0 / 15%)".into(),
                g1_accent: "#2563eb".into(),
                g2_accent: "#16a34a".into(),
                g3_accent: "#ea580c".into(),
                g4_accent: "#9333ea".into(),
            },
        },
        Theme {
            id: "light".into(),
            name: "Light".into(),
            builtin: true,
            tokens: ThemeTokens {
                title_bar: "#f3f4f6".into(),
                app_bg: "#ffffff".into(),
                panel_bg: "#f9fafb".into(),
                text: "#111827".into(),
                text_muted: "#6b7280".into(),
                border: "#e5e7eb".into(),
                hover_bg: "rgba(0,0,0,0.06)".into(),
                g1_accent: "#2563eb".into(),
                g2_accent: "#16a34a".into(),
                g3_accent: "#ea580c".into(),
                g4_accent: "#9333ea".into(),
            },
        },
        Theme {
            id: "pink".into(),
            name: "Pink".into(),
            builtin: true,
            tokens: ThemeTokens {
                title_bar: "#f9a8d4".into(),
                app_bg: "#fdf2f8".into(),
                panel_bg: "#fce7f3".into(),
                text: "#500724".into(),
                text_muted: "#9d174d".into(),
                border: "#f9a8d4".into(),
                hover_bg: "rgba(236,72,153,0.15)".into(),
                g1_accent: "#ec4899".into(),
                g2_accent: "#f472b6".into(),
                g3_accent: "#db2777".into(),
                g4_accent: "#be185d".into(),
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_themes_has_three() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 3);
        assert!(themes.iter().all(|t| t.builtin));
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"dark"));
        assert!(ids.contains(&"light"));
        assert!(ids.contains(&"pink"));
    }

    #[test]
    fn test_theme_tokens_round_trip_json() {
        let theme = &builtin_themes()[0];
        let json = serde_json::to_string(&theme).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, theme.id);
        assert_eq!(back.tokens.g3_accent, theme.tokens.g3_accent);
    }

    #[test]
    fn test_export_envelope_serializes() {
        let theme = &builtin_themes()[0];
        let envelope = ThemeExport {
            schema: THEME_SCHEMA.into(),
            name: theme.name.clone(),
            tokens: theme.tokens.clone(),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("gkey-theme-v1"));
        let back: ThemeExport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.schema, THEME_SCHEMA);
    }
}
