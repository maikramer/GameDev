//! The Tailwind colour palette, resolved by name.
//!
//! `slate-900`, `rose-400/80`, `amber` — the names a HUD designer already has
//! in their head, in every stylesheet declaration that takes a colour
//! (`background: slate-900/80`). Numbers are the Tailwind shades 50–950; a
//! bare hue name resolves to its 500 shade, and the `/N` suffix is a
//! percentage of alpha, exactly like Tailwind's opacity modifier.
//!
//! The table is the Tailwind v3 default palette verbatim, so hex values picked
//! from the Tailwind docs render identically here.

use bevy::prelude::Color;

/// Shades in table order: 50, 100, 200, … 950.
pub const SHADES: [u16; 11] = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950];

/// `(hue, [50, 100, …, 950])` — Tailwind v3 default palette, hex `#rrggbb`.
pub static PALETTE: &[(&str, [&str; 11])] = &[
    (
        "slate",
        [
            "#f8fafc", "#f1f5f9", "#e2e8f0", "#cbd5e1", "#94a3b8", "#64748b", "#475569", "#334155",
            "#1e293b", "#0f172a", "#020617",
        ],
    ),
    (
        "gray",
        [
            "#f9fafb", "#f3f4f6", "#e5e7eb", "#d1d5db", "#9ca3af", "#6b7280", "#4b5563", "#374151",
            "#1f2937", "#111827", "#030712",
        ],
    ),
    (
        "zinc",
        [
            "#fafafa", "#f4f4f5", "#e4e4e7", "#d4d4d8", "#a1a1aa", "#71717a", "#52525b", "#3f3f46",
            "#27272a", "#18181b", "#09090b",
        ],
    ),
    (
        "neutral",
        [
            "#fafafa", "#f5f5f5", "#e5e5e5", "#d4d4d4", "#a3a3a3", "#737373", "#525252", "#404040",
            "#262626", "#171717", "#0a0a0a",
        ],
    ),
    (
        "stone",
        [
            "#fafaf9", "#f5f5f4", "#e7e5e4", "#d6d3d1", "#a8a29e", "#78716c", "#57534e", "#44403c",
            "#292524", "#1c1917", "#0c0a09",
        ],
    ),
    (
        "red",
        [
            "#fef2f2", "#fee2e2", "#fecaca", "#fca5a5", "#f87171", "#ef4444", "#dc2626", "#b91c1c",
            "#991b1b", "#7f1d1d", "#450a0a",
        ],
    ),
    (
        "orange",
        [
            "#fff7ed", "#ffedd5", "#fed7aa", "#fdba74", "#fb923c", "#f97316", "#ea580c", "#c2410c",
            "#9a3412", "#7c2d12", "#431407",
        ],
    ),
    (
        "amber",
        [
            "#fffbeb", "#fef3c7", "#fde68a", "#fcd34d", "#fbbf24", "#f59e0b", "#d97706", "#b45309",
            "#92400e", "#78350f", "#451a03",
        ],
    ),
    (
        "yellow",
        [
            "#fefce8", "#fef9c3", "#fef08a", "#fde047", "#facc15", "#eab308", "#ca8a04", "#a16207",
            "#854d0e", "#713f12", "#422006",
        ],
    ),
    (
        "lime",
        [
            "#f7fee7", "#ecfccb", "#d9f99d", "#bef264", "#a3e635", "#84cc16", "#65a30d", "#4d7c0f",
            "#3f6212", "#365314", "#1a2e05",
        ],
    ),
    (
        "green",
        [
            "#f0fdf4", "#dcfce7", "#bbf7d0", "#86efac", "#4ade80", "#22c55e", "#16a34a", "#15803d",
            "#166534", "#14532d", "#052e16",
        ],
    ),
    (
        "emerald",
        [
            "#ecfdf5", "#d1fae5", "#a7f3d0", "#6ee7b7", "#34d399", "#10b981", "#059669", "#047857",
            "#065f46", "#064e3b", "#022c22",
        ],
    ),
    (
        "teal",
        [
            "#f0fdfa", "#ccfbf1", "#99f6e4", "#5eead4", "#2dd4bf", "#14b8a6", "#0d9488", "#0f766e",
            "#115e59", "#134e4a", "#042f2e",
        ],
    ),
    (
        "cyan",
        [
            "#ecfeff", "#cffafe", "#a5f3fc", "#67e8f9", "#22d3ee", "#06b6d4", "#0891b2", "#0e7490",
            "#155e75", "#164e63", "#083344",
        ],
    ),
    (
        "sky",
        [
            "#f0f9ff", "#e0f2fe", "#bae6fd", "#7dd3fc", "#38bdf8", "#0ea5e9", "#0284c7", "#0369a1",
            "#075985", "#0c4a6e", "#082f49",
        ],
    ),
    (
        "blue",
        [
            "#eff6ff", "#dbeafe", "#bfdbfe", "#93c5fd", "#60a5fa", "#3b82f6", "#2563eb", "#1d4ed8",
            "#1e40af", "#1e3a8a", "#172554",
        ],
    ),
    (
        "indigo",
        [
            "#eef2ff", "#e0e7ff", "#c7d2fe", "#a5b4fc", "#818cf8", "#6366f1", "#4f46e5", "#4338ca",
            "#3730a3", "#312e81", "#1e1b4b",
        ],
    ),
    (
        "violet",
        [
            "#f5f3ff", "#ede9fe", "#ddd6fe", "#c4b5fd", "#a78bfa", "#8b5cf6", "#7c3aed", "#6d28d9",
            "#5b21b6", "#4c1d95", "#2e1065",
        ],
    ),
    (
        "purple",
        [
            "#faf5ff", "#f3e8ff", "#e9d5ff", "#d8b4fe", "#c084fc", "#a855f7", "#9333ea", "#7e22ce",
            "#6b21a8", "#581c87", "#3b0764",
        ],
    ),
    (
        "fuchsia",
        [
            "#fdf4ff", "#fae8ff", "#f5d0fe", "#f0abfc", "#e879f9", "#d946ef", "#c026d3", "#a21caf",
            "#86198f", "#701a75", "#4a044e",
        ],
    ),
    (
        "pink",
        [
            "#fdf2f8", "#fce7f3", "#fbcfe8", "#f9a8d4", "#f472b6", "#ec4899", "#db2777", "#be185d",
            "#9d174d", "#831843", "#500724",
        ],
    ),
    (
        "rose",
        [
            "#fff1f2", "#ffe4e6", "#fecdd3", "#fda4af", "#fb7185", "#f43f5e", "#e11d48", "#be123c",
            "#9f1239", "#881337", "#4c0519",
        ],
    ),
];

/// Resolves a palette name to a colour: `rose-400`, `slate-900/80`, `amber`.
///
/// Duas formas de alpha: o modificador `/N` da Tailwind (`rose-500/25`) ou o
/// par hex colado (`stone-900e0` = shade 900 com alpha `e0`) — quem escreve
/// CSS deve cores hex a vida inteira; o teclado segue o hábito. Valores de
/// alpha clampeiam a 0–1. `None` quando o nome não está na paleta.
pub fn resolve(name: &str) -> Option<Color> {
    let name = name.trim().to_ascii_lowercase();
    // `hue-shade/alpha`, `hue-shade`, `hue`.
    let (head, slash_alpha) = match name.split_once('/') {
        Some((head, alpha)) => (
            head,
            Some((alpha.trim().parse::<f32>().ok()? / 100.0).clamp(0.0, 1.0)),
        ),
        None => (name.as_str(), None),
    };
    let (hue, shade_index, hex_alpha) = match head.split_once('-') {
        Some((hue, shade)) => {
            // `900e0` — shade de 3 dígitos com alpha hex colado.
            let (shade_text, hex_alpha) = match shade.len() {
                5 if shade.is_ascii() && shade[..3].chars().all(|c| c.is_ascii_digit()) => {
                    (&shade[..3], Some(&shade[3..]))
                }
                _ => (shade, None),
            };
            let shade: u16 = shade_text.parse().ok()?;
            let index = SHADES.iter().position(|s| *s == shade)?;
            (hue, index, hex_alpha)
        }
        None => (head, 5, None), // a bare hue is its 500 shade
    };
    let hex = PALETTE
        .iter()
        .find(|(name, _)| *name == hue)
        .map(|(_, shades)| shades[shade_index])?;
    let mut color = parse_hex6(hex)?;
    let alpha = match (slash_alpha, hex_alpha) {
        (Some(alpha), _) => alpha,
        (None, Some(hex_alpha)) => u8::from_str_radix(hex_alpha, 16).ok()? as f32 / 255.0,
        (None, None) => 1.0,
    };
    if alpha < 1.0 {
        let mut srgba = color.to_srgba();
        srgba.alpha *= alpha;
        color = Color::Srgba(srgba);
    }
    Some(color)
}

/// `#rrggbb` → [`Color`] — the only form the table stores.
fn parse_hex6(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| -> Option<f32> {
        Some(u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()? as f32 / 255.0)
    };
    Some(Color::srgb(byte(0)?, byte(1)?, byte(2)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shades_and_bare_hues_resolve() {
        let slate = resolve("slate-900").expect("slate-900");
        assert!((slate.to_srgba().red - 0x0f as f32 / 255.0).abs() < 1e-5);
        // A bare hue is its 500 shade.
        let rose = resolve("ROSE").expect("rose");
        let rose500 = resolve("rose-500").expect("rose-500");
        assert_eq!(rose, rose500);
    }

    #[test]
    fn test_alpha_modifier_multiplies() {
        let solid = resolve("rose-500").expect("solid");
        let dim = resolve("rose-500/50").expect("dim");
        assert!((solid.to_srgba().alpha - 1.0).abs() < 1e-6);
        assert!((dim.to_srgba().alpha - 0.5).abs() < 1e-6);
        // RGB is untouched; only alpha moves.
        assert!((solid.to_srgba().red - dim.to_srgba().red).abs() < 1e-6);
        // Percentagens absurdas clampeiam — não fabricam alpha > 1.
        assert!((resolve("rose-500/101").expect("clamped").to_srgba().alpha - 1.0).abs() < 1e-6);
        assert!((resolve("rose-500/-5").expect("clamped").to_srgba().alpha).abs() < 1e-6);
        // Alpha hex COLADO ao shade: `stone-900e0` = 900 com alpha e0.
        let hexed = resolve("stone-900e0").expect("hex alpha");
        assert!((hexed.to_srgba().alpha - 0xe0 as f32 / 255.0).abs() < 1e-6);
        let solid900 = resolve("stone-900").expect("solid900");
        assert!((solid900.to_srgba().red - hexed.to_srgba().red).abs() < 1e-6);
        // Shade que começa por dígito mas não é de 3 não se confunde.
        assert!(resolve("slate-950f0").is_some(), "950 + f0 colado");
    }

    #[test]
    fn test_unknown_names_are_not_guessed() {
        assert!(resolve("slate-999").is_none());
        assert!(resolve("chartreuse-500").is_none());
        assert!(resolve("rose-").is_none());
        // A non-ASCII shade can never reach the byte slicing: `None`, not a
        // panic on a cut char boundary.
        assert!(resolve("slate-aaáb").is_none());
        assert!(resolve("slate-9€a").is_none());
    }

    #[test]
    fn test_every_entry_is_a_well_formed_hex6() {
        for (hue, shades) in PALETTE {
            assert_eq!(shades.len(), SHADES.len(), "{hue}");
            for hex in shades {
                assert!(parse_hex6(hex).is_some(), "{hue} {hex}");
            }
        }
    }
}
