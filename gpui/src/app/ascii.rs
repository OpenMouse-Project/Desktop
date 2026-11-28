//! The ASCII noise backdrop behind every page.

use super::*;

/// Background art: a static ASCII density ramp painted behind the shell.
/// Static (no animation), seeded once per launch, and only regenerated when the
/// window's character grid changes size.
pub(super) const ASCII_RAMP: &[u8] = b" .:-=+*x#%@";

/// Cell width as a fraction of the font size. Monospace advances run from
/// ~0.55em (Consolas) to ~0.6em (Menlo, DejaVu); deliberately under all of
/// them, so the trailing columns overflow the window and get clipped instead
/// of leaving a strip of bare background on the right.
pub(super) const ASCII_CELL_RATIO: f32 = 0.5;

/// Ink opacity at the corner the field starts from.
pub(super) const ASCII_ALPHA: f32 = 0.09;

/// Alpha multiplier on the OLED theme, where the field would otherwise vanish.
pub(super) const ASCII_OLED_GAIN: f32 = 2.2;

/// The field slides back in from the right whenever the theme changes.
pub(super) const ASCII_SWEEP_MS: u64 = 520;

/// How far a row starts to the right of its resting place, in px.
pub(super) const ASCII_SWEEP_OFFSET: f32 = 220.0;

/// Share of the sweep spent waiting for the wave to reach the last row, so
/// rows arrive top-down instead of all at once.
pub(super) const ASCII_SWEEP_STAGGER: f32 = 0.55;

/// How far the falloff reaches, as a fraction of the corner-to-opposite
/// corner distance.
pub(super) const ASCII_FADE_REACH: f32 = 0.9;

/// Number of alpha bands per row; each is its own text run.
pub(super) const ASCII_BANDS: usize = 6;

/// Falloff from the top-right corner, slightly wider than tall. `u` runs
/// left→right, `v` top→bottom; returns 0..1.
pub(super) fn ascii_falloff(u: f32, v: f32) -> f32 {
    let dx = 1.0 - u;
    let distance = (dx * dx + v * v * 0.8).sqrt();
    let s = (1.0 - distance / ASCII_FADE_REACH).clamp(0.0, 1.0);
    // Smoothstep: no hard edge where the field reaches zero.
    s * s * (3.0 - 2.0 * s)
}

/// Ink opacity for one cell region. The alpha fades out with the density in
/// both directions, so the gradient dims the layer instead of veiling the
/// whole window at one flat opacity.
pub(super) fn ascii_cell_alpha(u: f32, v: f32) -> f32 {
    ASCII_ALPHA * ascii_falloff(u, v).sqrt()
}

/// Deterministic lattice hash → 0..1.
pub(super) fn ascii_hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Smooth value noise, 0..1.
pub(super) fn ascii_value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (xi as i32, yi as i32);
    let top = ascii_hash(xi, yi) + (ascii_hash(xi + 1, yi) - ascii_hash(xi, yi)) * sx;
    let bottom = ascii_hash(xi, yi + 1) + (ascii_hash(xi + 1, yi + 1) - ascii_hash(xi, yi + 1)) * sx;
    top + (bottom - top) * sy
}

/// Three-octave fractal noise, 0..1, in cell units (cells are ~2x taller
/// than wide, so `y` is scaled harder than `x`).
pub(super) fn ascii_fbm(x: f32, y: f32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..3 {
        let o = octave as f32 * 17.0;
        sum += amp * ascii_value_noise(x * freq + o, y * freq - o);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// Interleaved gradient noise, 0..1: an unstructured dither without the
/// visible grid a Bayer matrix leaves on flat areas.
pub(super) fn ascii_dither(x: usize, y: usize) -> f32 {
    (52.982_92 * ((0.067_110_56 * x as f32 + 0.005_837_15 * y as f32).fract())).fract()
}

/// Where in the noise field this launch samples, so the clouds differ every
/// time the app starts. Fixed for the life of the process: resizing the window
/// regenerates the grid and must not make the pattern jump.
pub(super) fn ascii_seed() -> (f32, f32) {
    static SEED: std::sync::OnceLock<(f32, f32)> = std::sync::OnceLock::new();
    *SEED.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u32);
        let hash = |salt: i32| ascii_hash(nanos as i32, salt) * 1000.0;
        (hash(1), hash(2))
    })
}

/// Build one character row per screen row. Density is highest at the
/// top-right corner and falls away toward the bottom-left; fractal noise
/// breaks the gradient into cloud-like contours instead of a clean radial.
/// The noise is offset by [`ascii_seed`], so each launch gets its own clouds.
pub(super) fn ascii_background_rows(cols: usize, rows: usize) -> Vec<SharedString> {
    let last = (ASCII_RAMP.len() - 1) as f32;
    let (seed_x, seed_y) = ascii_seed();
    let mut out = Vec::with_capacity(rows);
    for y in 0..rows {
        let v = (y as f32 + 0.5) / rows as f32;
        let mut line = String::with_capacity(cols);
        for x in 0..cols {
            let u = (x as f32 + 0.5) / cols as f32;
            let cloud = ascii_fbm(x as f32 * 0.07 + seed_x, y as f32 * 0.14 + seed_y);
            let t = (ascii_falloff(u, v) * (0.3 + 1.2 * cloud)).clamp(0.0, 1.0).powf(1.25);
            let dithered = t * last + (ascii_dither(x, y) - 0.5) * 0.9;
            let index = dithered.round().clamp(0.0, last) as usize;
            line.push(ASCII_RAMP[index] as char);
        }
        out.push(line.into());
    }
    out
}

impl AppView {
    /// Subtle ASCII field behind the whole shell. Rows are cached by grid
    /// size; the accent color is applied at paint time, so switching themes
    /// recolors the field without regenerating it. A theme change also
    /// replays the entrance — the art itself carries the transition, sliding
    /// back in from the right with the top row arriving first.
    pub(super) fn render_ascii_background(&mut self, window: &Window, cx: &Context<Self>) -> AnyElement {
        // Kit's mono family resolves per platform (Menlo / Consolas /
        // DejaVu) and falls back to an installed family, so the grid always
        // has a real monospace face whose advance it can rely on.
        let font_size = cx.theme().mono_font_size;
        let line_height = font_size.as_f32();
        let viewport = window.viewport_size();
        let cols = ((viewport.width.as_f32() / (line_height * ASCII_CELL_RATIO)).ceil() as usize).max(1);
        let grid_rows = ((viewport.height.as_f32() / line_height).ceil() as usize).max(1);
        if self.ascii_dims != (cols, grid_rows) {
            self.ascii_rows = ascii_background_rows(cols, grid_rows);
            self.ascii_dims = (cols, grid_rows);
        }
        let font_family = cx.theme().mono_font_family.clone();
        let color = cx.theme().primary;
        // On true black the same ink reads far fainter than on the lifted
        // greys of the other themes, so it gets extra gain there.
        let gain = if cx.theme().background.l < 0.03 { ASCII_OLED_GAIN } else { 1.0 };
        let total = self.ascii_rows.len();
        let rows: Vec<(SharedString, f32)> = self
            .ascii_rows
            .iter()
            .enumerate()
            .map(|(index, line)| (line.clone(), (index as f32 + 0.5) / total as f32))
            .collect();
        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .w_full()
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .with_animation(
                // The id carries the theme generation: a new one restarts the
                // entrance instead of leaving the field parked mid-frame.
                format!("ascii-field-{}", self.theme_seq),
                Animation::new(Duration::from_millis(ASCII_SWEEP_MS)),
                move |field, delta| {
                    field.children(rows.iter().map(|(line, position)| {
                        // The wave reaches each row in turn; a row holds its
                        // start offset until then.
                        let local = ((delta - position * ASCII_SWEEP_STAGGER)
                            / (1.0 - ASCII_SWEEP_STAGGER))
                            .clamp(0.0, 1.0);
                        let eased = 1.0 - (1.0 - local).powi(3);
                        div()
                            .h(px(line_height))
                            .flex_none()
                            .left(px((1.0 - eased) * ASCII_SWEEP_OFFSET))
                            .font_family(font_family.clone())
                            .text_size(font_size)
                            .line_height(px(line_height))
                            .whitespace_nowrap()
                            .child(div().flex().flex_none().children(
                                (0..ASCII_BANDS).filter_map(|band| {
                                    let start = band * line.len() / ASCII_BANDS;
                                    let end = (band + 1) * line.len() / ASCII_BANDS;
                                    if start == end {
                                        return None;
                                    }
                                    let u = ((start + end) as f32 * 0.5) / line.len() as f32;
                                    Some(
                                        div()
                                            .flex_none()
                                            .text_color(Hsla {
                                                a: (ascii_cell_alpha(u, *position) * gain).min(1.0),
                                                ..color
                                            })
                                            .child(SharedString::from(line[start..end].to_string())),
                                    )
                                }),
                            ))
                    }))
                },
            )
            .into_any_element()
    }
}
