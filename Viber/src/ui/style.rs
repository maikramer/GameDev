//! CSS-like stylesheets for the declarative UI.
//!
//! A `<UiStyle>` block in a world holds plain text rules:
//!
//! ```text
//! .panel        { background: #14120fdd; radius: 14; padding: 10 8 }
//! .panel:hover  { background: #241f18ee }
//! #hp-fill      { background: #4ade80; height: 10 }
//! UiText        { color: #f0ece0; font-size: 14 }
//! ```
//!
//! The dialect is deliberately small and *flat*: one simple selector per rule
//! (tag, `.class`, `#id`, optionally with a `:hover` / `:active` / `:disabled`
//! state), declarations separated by `;`, values written the way a game author
//! would write them (bare numbers are pixels). There are no combinators,
//! no cascade of inherited properties and no units beyond `px` / `%` — every
//! feature here has to survive being read by someone who has never seen this
//! file, which is the whole point of authoring a HUD in XML instead of Rust.
//!
//! Resolution order (later wins): tag → class → id → inline `style="…"`, and
//! within one specificity level, source order. State rules (`:hover`) count as
//! a class, exactly like CSS.

use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::{
    GridAutoFlow, GridPlacement, GridTrack, GridTrackRepetition, RepeatedGridTrack, ShadowStyle,
};
use bevy::window::{CursorIcon, SystemCursorIcon};

use super::palette;

/// Interaction state a rule can be scoped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StyleState {
    /// Applies always.
    #[default]
    Normal,
    /// Pointer is over the element.
    Hover,
    /// Pointer is pressed on the element.
    Active,
    /// Element carries the `disabled` flag.
    Disabled,
}

/// One compound selector: an optional tag, any number of classes, an optional
/// id and an optional state — `UiPanel.card.wide#hero:hover`.
///
/// Empty everywhere means `*`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Compound {
    pub tag: Option<String>,
    pub classes: Vec<String>,
    pub id: Option<String>,
    pub state: StyleState,
}

impl Compound {
    /// CSS specificity: id 100, class (and state pseudo) 10, tag 1.
    pub fn specificity(&self) -> u32 {
        let mut score = 0;
        if self.id.is_some() {
            score += 100;
        }
        score += 10 * self.classes.len() as u32;
        if self.state != StyleState::Normal {
            score += 10;
        }
        if self.tag.is_some() {
            score += 1;
        }
        score
    }

    /// Does this compound describe `element`?
    pub fn matches(&self, element: &ElementRef<'_>) -> bool {
        if self.state != StyleState::Normal && self.state != element.state {
            return false;
        }
        if let Some(tag) = &self.tag {
            if tag != element.tag {
                return false;
            }
        }
        if let Some(id) = &self.id {
            if element.id != Some(id.as_str()) {
                return false;
            }
        }
        self.classes
            .iter()
            .all(|class| element.classes.iter().any(|c| c == class))
    }
}

/// What the cascade knows about one element while resolving.
#[derive(Debug, Clone, Copy)]
pub struct ElementRef<'a> {
    pub tag: &'a str,
    pub id: Option<&'a str>,
    pub classes: &'a [String],
    pub state: StyleState,
}

/// A full selector: compounds separated by whitespace, read as the CSS
/// descendant combinator (`.panel .fill` = a `.fill` anywhere inside a
/// `.panel`). The last compound describes the element itself.
///
/// Child (`>`), sibling and attribute combinators are deliberately absent: a
/// HUD is a shallow tree, and descendant matching plus classes has covered
/// every rule the example needed without the cost of a real selector engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub parts: Vec<Compound>,
}

impl Selector {
    pub fn specificity(&self) -> u32 {
        self.parts.iter().map(Compound::specificity).sum()
    }

    /// Matches against an ancestor chain ordered root-first, with the element
    /// itself last.
    ///
    /// Walks both back-to-front so the ancestors can be skipped freely, which
    /// is exactly what "descendant, at any depth" means.
    pub fn matches(&self, chain: &[ElementRef<'_>]) -> bool {
        let Some((last, ancestors)) = self.parts.split_last() else {
            return false;
        };
        let Some((element, chain_ancestors)) = chain.split_last() else {
            return false;
        };
        if !last.matches(element) {
            return false;
        }
        let mut remaining = chain_ancestors;
        for part in ancestors.iter().rev() {
            match remaining
                .iter()
                .rposition(|candidate| part.matches(candidate))
            {
                Some(index) => remaining = &remaining[..index],
                None => return false,
            }
        }
        true
    }
}

/// A parsed `selector { declarations }` block.
#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: Selector,
    pub props: StyleProps,
    /// Position in the source, breaking specificity ties.
    pub order: usize,
    /// `@media` condition the rule lives under, if any. Rules whose condition
    /// fails against the current window are invisible to the cascade — the
    /// mechanism behind responsive layouts.
    pub media: Option<MediaCond>,
    /// Índice da folha que produziu a regra (`parse_into`) — uma `UiRoot`
    /// só vê as folhas que lhe foram atribuídas no build (shadow-DOM-lite).
    pub sheet: usize,
}

/// Uma condição `@media (…)` — todos os predicados têm de dar verdade.
///
/// Suporta `min-width`/`max-width`/`min-height`/`max-height` (píxeis lógicos),
/// `min-aspect`/`max-aspect` (largura ÷ altura) e as palavras `portrait`
/// (proporção < 1) e `landscape` (≥ 1), ligadas por `and`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaCond {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub min_aspect: Option<f32>,
    pub max_aspect: Option<f32>,
    pub portrait: bool,
    pub landscape: bool,
}

impl MediaCond {
    /// `true` quando a janela (píxeis lógicos) satisfaz a condição inteira.
    pub fn matches(&self, width: f32, height: f32) -> bool {
        let aspect = if height > 0.0 { width / height } else { 1.0 };
        self.min_width.is_none_or(|v| width >= v)
            && self.max_width.is_none_or(|v| width <= v)
            && self.min_height.is_none_or(|v| height >= v)
            && self.max_height.is_none_or(|v| height <= v)
            && self.min_aspect.is_none_or(|v| aspect >= v)
            && self.max_aspect.is_none_or(|v| aspect <= v)
            && (!self.portrait || aspect < 1.0)
            && (!self.landscape || aspect >= 1.0)
    }
}

/// `(min-width: 900) and (portrait)` → [`MediaCond`]; `None` = ilegível.
pub fn parse_media_cond(text: &str) -> Option<MediaCond> {
    let mut cond = MediaCond::default();
    // " and " com espaços — um split por "and" a seco partia "landscape".
    for part in text.split(" and ") {
        let part = part
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')')
            .trim();
        if part.is_empty() {
            continue;
        }
        let (name, value) = match part.split_once(':') {
            Some((n, v)) => (n.trim().to_ascii_lowercase(), Some(v.trim())),
            None => (part.to_ascii_lowercase(), None),
        };
        let number = |v: Option<&str>| -> Option<f32> { v.and_then(parse_number) };
        match name.as_str() {
            "min-width" => cond.min_width = Some(number(value)?),
            "max-width" => cond.max_width = Some(number(value)?),
            "min-height" => cond.min_height = Some(number(value)?),
            "max-height" => cond.max_height = Some(number(value)?),
            "min-aspect" => cond.min_aspect = Some(number(value)?),
            "max-aspect" => cond.max_aspect = Some(number(value)?),
            "portrait" => cond.portrait = true,
            "landscape" => cond.landscape = true,
            _ => return None,
        }
    }
    Some(cond)
}

/// Every styleable property, each `None` when the rule does not set it.
///
/// Tamanho de fonte quando NEM o elemento nem nenhum ancestral declara um —
/// o mesmo omissão do browser (e do `TextFont` da Bevy).
pub const DEFAULT_FONT_PX: f32 = 16.0;

/// Uma medida de estilo: um [`Val`] simples ou uma EXPRESSÃO (`calc(…)`,
/// `min(…)`, `max(…)`, `clamp(…)`) avaliada contra o viewport autoral no
/// `resolve_viewport` — os píxeis da expressão falam o MESMO espaço
/// (janela÷escala) que tudo o resto. Subconjunto do CSS documentado: número
/// nu = px; `%` dentro de expressões não é suportado (depende do pai).
#[derive(Debug, Clone, PartialEq)]
pub enum Measure {
    Plain(Val),
    Expr(Expr),
}

impl Measure {
    pub fn plain(val: Val) -> Self {
        Self::Plain(val)
    }

    /// O [`Val`] por baixo. Depois de `resolve_viewport` toda a medida é
    /// `Plain`, por isso `Expr` nunca chega ao layout (cai em `Auto`).
    pub fn to_val(&self) -> Val {
        match self {
            Self::Plain(v) => *v,
            Self::Expr(_) => Val::Auto,
        }
    }
}

/// Expressão aritmética de medidas — o subconjunto do `calc()` que um HUD
/// precisa, com precedência normal e parêntesis.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Px(f32),
    Vw(f32),
    Vh(f32),
    VMin(f32),
    VMax(f32),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    /// CSS exige número × medida; aqui multiplica-se o que vier (subconjunto).
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Min(Vec<Expr>),
    Max(Vec<Expr>),
    /// `clamp(min, valor, max)`.
    Clamp(Box<Expr>, Box<Expr>, Box<Expr>),
}

impl Expr {
    /// Avalia contra o viewport autoral; `None` = divisão por zero.
    pub fn eval(&self, viewport: (f32, f32)) -> Option<f32> {
        let (vw, vh) = (viewport.0.max(1.0), viewport.1.max(1.0));
        match self {
            Self::Px(v) => Some(*v),
            Self::Vw(v) => Some(vw * v / 100.0),
            Self::Vh(v) => Some(vh * v / 100.0),
            Self::VMin(v) => Some(vw.min(vh) * v / 100.0),
            Self::VMax(v) => Some(vw.max(vh) * v / 100.0),
            Self::Add(a, b) => Some(a.eval(viewport)? + b.eval(viewport)?),
            Self::Sub(a, b) => Some(a.eval(viewport)? - b.eval(viewport)?),
            Self::Mul(a, b) => Some(a.eval(viewport)? * b.eval(viewport)?),
            Self::Div(a, b) => {
                let divisor = b.eval(viewport)?;
                if divisor.abs() < 1e-6 {
                    warn!("ui style: divisão por zero em calc() — declaração descartada");
                    None
                } else {
                    Some(a.eval(viewport)? / divisor)
                }
            }
            Self::Min(args) => args
                .iter()
                .map(|a| a.eval(viewport))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .reduce(f32::min),
            Self::Max(args) => args
                .iter()
                .map(|a| a.eval(viewport))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .reduce(f32::max),
            Self::Clamp(min, value, max) => {
                let low = min.eval(viewport)?;
                let mid = value.eval(viewport)?;
                let high = max.eval(viewport)?;
                Some(mid.max(low).min(high))
            }
        }
    }
}

/// Aceita `calc(…)`/`min(…)`/`max(…)`/`clamp(…)` ou um Val simples — é este o
/// parser dos campos de medida escalares.
pub fn parse_measure(value: &str) -> Option<Measure> {
    let text = value.trim();
    let lower = text.to_ascii_lowercase();
    for name in ["calc", "min", "max", "clamp"] {
        if lower.starts_with(name) && lower[name.len()..].trim_start().starts_with('(') {
            let inner = text[name.len()..].trim();
            let inner = inner.strip_prefix('(')?.strip_suffix(')')?.trim();
            return parse_expr_function(name, inner).map(Measure::Expr);
        }
    }
    parse_val(text).map(Measure::Plain)
}

/// `calc(expr)`, `min(a, b, …)`, `max(a, b, …)`, `clamp(min, valor, max)`.
fn parse_expr_function(name: &str, args_text: &str) -> Option<Expr> {
    let mut args = split_top_level_commas(args_text)
        .into_iter()
        .map(|arg| parse_expression(arg.trim()))
        .collect::<Option<Vec<_>>>()?;
    match name {
        "calc" if args.len() == 1 => Some(args.remove(0)),
        "min" if !args.is_empty() => Some(Expr::Min(args)),
        "max" if !args.is_empty() => Some(Expr::Max(args)),
        "clamp" if args.len() == 3 => Some(Expr::Clamp(
            Box::new(args.remove(0)),
            Box::new(args.remove(0)),
            Box::new(args.remove(0)),
        )),
        _ => {
            warn!(
                "ui style: {name}({args_text}) não fecha com a aridade certa — declaração saltada"
            );
            None
        }
    }
}

/// Divide nos vírgulas de TOPO (vírgulas dentro de parêntesis não contam).
fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// Recursive descent: expr := term (('+'|'-') term)*  —  term := unary (('*'|'/') unary)*.
fn parse_expression(text: &str) -> Option<Expr> {
    let mut parser = ExprParser {
        rest: text,
        position: 0,
    };
    let expr = parser.additive()?;
    parser.skip_spaces();
    if parser.rest.is_empty() {
        Some(expr)
    } else {
        warn!(
            "ui style: sobrou `{}` numa expressão — declaração saltada",
            parser.rest
        );
        None
    }
}

struct ExprParser<'a> {
    rest: &'a str,
    position: usize,
}

impl ExprParser<'_> {
    fn skip_spaces(&mut self) {
        let trimmed = self.rest.trim_start();
        self.position += self.rest.len() - trimmed.len();
        self.rest = trimmed;
    }

    fn eat(&mut self, token: char) -> bool {
        self.skip_spaces();
        if self.rest.starts_with(token) {
            self.rest = &self.rest[1..];
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn additive(&mut self) -> Option<Expr> {
        let mut left = self.multiplicative()?;
        loop {
            self.skip_spaces();
            // Fim da string NÃO é erro — é o fim da expressão (o `?` abaixo
            // só rejeita quando sobra lixo que não é operador).
            let Some(next) = self.rest.chars().next() else {
                return Some(left);
            };
            let op = match next {
                '+' => Expr::Add as fn(_, _) -> _,
                '-' => Expr::Sub as fn(_, _) -> _,
                _ => return Some(left),
            };
            self.rest = &self.rest[1..];
            self.position += 1;
            let right = self.multiplicative()?;
            left = op(Box::new(left), Box::new(right));
        }
    }

    fn multiplicative(&mut self) -> Option<Expr> {
        let mut left = self.unary()?;
        loop {
            self.skip_spaces();
            let Some(next) = self.rest.chars().next() else {
                return Some(left);
            };
            let op = match next {
                '*' => Expr::Mul as fn(_, _) -> _,
                '/' => Expr::Div as fn(_, _) -> _,
                _ => return Some(left),
            };
            self.rest = &self.rest[1..];
            self.position += 1;
            let right = self.unary()?;
            left = op(Box::new(left), Box::new(right));
        }
    }

    /// `-expr` unário e primários (número com unidade, função, parêntesis).
    fn unary(&mut self) -> Option<Expr> {
        self.skip_spaces();
        if self.eat('-') {
            let value = self.unary()?;
            return Some(Expr::Sub(Box::new(Expr::Px(0.0)), Box::new(value)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Option<Expr> {
        self.skip_spaces();
        if self.eat('(') {
            let expr = self.additive()?;
            if !self.eat(')') {
                warn!("ui style: parêntesis por fechar numa expressão — declaração saltada");
                return None;
            }
            return Some(expr);
        }
        // Função aninhada: min(...)/max(...)/calc(...) dentro da expressão.
        let ident: String = self
            .rest
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if ["min", "max", "calc", "clamp"].contains(&ident.as_str()) {
            let after = &self.rest[ident.len()..];
            if after.trim_start().starts_with('(') {
                let open_offset = self.rest.len() - after.len() + after.find('(')?;
                let inner = &self.rest[open_offset + 1..];
                let close = find_matching_paren(inner)?;
                let expr = parse_expr_function(&ident, &inner[..close])?;
                let consumed = open_offset + 1 + close + 1;
                self.position += consumed;
                self.rest = &self.rest[consumed..];
                return Some(expr);
            }
        }
        // Número com unidade opcional (número nu = px, regra do dialecto).
        let number_len = self
            .rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .map(char::len_utf8)
            .sum::<usize>();
        let number: f32 = self.rest[..number_len].parse().ok()?;
        self.rest = &self.rest[number_len..];
        self.position += number_len;
        let unit_len = self
            .rest
            .chars()
            .take_while(|c| c.is_ascii_alphabetic() || *c == '%')
            .map(char::len_utf8)
            .sum::<usize>();
        let (unit, rest) = self.rest.split_at(unit_len);
        self.rest = rest;
        self.position += unit_len;
        match unit.to_ascii_lowercase().as_str() {
            "" | "px" => Some(Expr::Px(number)),
            "vw" => Some(Expr::Vw(number)),
            "vh" => Some(Expr::Vh(number)),
            "vmin" => Some(Expr::VMin(number)),
            "vmax" => Some(Expr::VMax(number)),
            "%" => {
                warn!(
                    "ui style: `%` dentro de calc() precisa do tamanho do PAI — não suportado; use vw/vh/px"
                );
                None
            }
            other => {
                warn!(
                    "ui style: unidade `{other}` desconhecida numa expressão — declaração saltada"
                );
                None
            }
        }
    }
}

/// This is a *patch*, not a full style: rules are merged in cascade order and
/// only the fields a rule mentions overwrite the ones below it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleProps {
    // ── layout ──────────────────────────────────────────────────────────
    pub display: Option<Display>,
    pub position: Option<PositionType>,
    pub flex_direction: Option<FlexDirection>,
    pub flex_wrap: Option<FlexWrap>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignSelf>,
    pub justify_content: Option<JustifyContent>,
    // Medidas escalares — podem carregar EXPRESSÕES (`calc(…)`) que só
    // fecham contra o viewport autoral (ver [`Measure`]). Rects
    // (padding/margin/border) e raios ficam em Val simples: aceitam px, %,
    // vw/vh/vmin/vmax — o que a responsividade de espaçamentos pede.
    pub width: Option<Measure>,
    pub height: Option<Measure>,
    pub min_width: Option<Measure>,
    pub min_height: Option<Measure>,
    pub max_width: Option<Measure>,
    pub max_height: Option<Measure>,
    pub top: Option<Measure>,
    pub right: Option<Measure>,
    pub bottom: Option<Measure>,
    pub left: Option<Measure>,
    pub padding: Option<UiRect>,
    pub margin: Option<UiRect>,
    pub border: Option<UiRect>,
    pub row_gap: Option<Measure>,
    pub column_gap: Option<Measure>,
    pub aspect_ratio: Option<f32>,
    pub overflow_clip: Option<bool>,
    // ── grid ────────────────────────────────────────────────────────────
    /// `display: grid` liga o layout de grelha; as pistas definem o tamanho
    /// das colunas/linhas (`grid-template-columns: repeat(4, 1fr) 24`).
    pub grid_template_columns: Option<Vec<RepeatedGridTrack>>,
    pub grid_template_rows: Option<Vec<RepeatedGridTrack>>,
    pub grid_auto_flow: Option<GridAutoFlow>,
    /// Colocação explícita do ITEM na grelha (`grid-column: 2 / span 3`).
    pub grid_column: Option<GridPlacement>,
    pub grid_row: Option<GridPlacement>,
    // ── paint ───────────────────────────────────────────────────────────
    pub background: Option<Color>,
    pub border_color: Option<Color>,
    pub border_radius: Option<BorderRadius>,
    /// `outline: <width> <colour>` — o contorno escuro que garante que um
    /// widget se lê sobre QUALQUER fundo (regra do crítico, r5).
    pub outline: Option<OutlineSpec>,
    /// Sombra(s) projetada(s) (`box-shadow: 0 4 12 #00000066`) — profundidade
    /// sem recortar fora do nó, ao contrário do `outline`.
    pub box_shadow: Option<Vec<ShadowStyle>>,
    /// Multiplica o alpha de background/text/image colours.
    pub opacity: Option<f32>,
    pub z_index: Option<i32>,
    /// Rotation in degrees, clockwise (`rotate: 45`).
    pub rotate: Option<f32>,
    /// Uniform scale (`scale: 1.2`).
    pub scale: Option<f32>,
    /// Offset em píxeis que NÃO conta para o layout (`translate: 0 -4`).
    pub translate: Option<Vec2>,
    /// Cursor do rato enquanto o ponteiro está sobre o elemento.
    pub cursor: Option<CursorIcon>,
    /// `pointer-events: none` — o elemento não recebe hover nem cliques.
    pub pointer_none: Option<bool>,
    // ── text ────────────────────────────────────────────────────────────
    pub color: Option<Color>,
    /// Fonte em qualquer unidade absoluta (`14`, `2vmin`, `1.5vh`,
    /// `clamp(14px, 2vmin, 22px)`) — viewport units e expressões fecham em px
    /// no [`StyleProps::resolve_viewport`]. Relativas (`120%`, `1.2em`) chegam
    /// como `Measure::Plain(Val::Percent)` e resolvem contra a fonte HERDADA
    /// do pai no momento de aplicar (runtime).
    pub font_size: Option<Measure>,
    /// `font-size: 1.2em` — fração da fonte computada do PAI.
    pub font_size_em: Option<f32>,
    /// `font-size: 0.9rem` — fração da fonte computada da RAIZ (`uiroot`).
    pub font_size_rem: Option<f32>,
    /// Espessura do tipo (`font-weight: bold` / 100–1000). Só tem efeito em
    /// fontes variáveis.
    pub font_weight: Option<f32>,
    /// `text-align` horizontal.
    pub text_align: Option<bevy::text::Justify>,
    /// Como o texto quebra ao ultrapassar a largura (`word`/`char`/`none`).
    pub linebreak: Option<LineBreak>,
    /// Sublinhado/riscado, com cor opcional (`Off` = desligar explicitamente
    /// algo herdado).
    pub text_underline: Option<Decoration>,
    pub text_strikethrough: Option<Decoration>,
    /// `text-shadow: <dx> <dy> <colour>` — `Some(None)` is an explicit
    /// `text-shadow: none`, which removes an inherited one.
    pub text_shadow: Option<Option<TextShadowSpec>>,
    // ── image ───────────────────────────────────────────────────────────
    pub image_tint: Option<Color>,
    pub image_mode: Option<NodeImageMode>,
}

/// Estado de um sublinhado/riscado: desligado, ligado com a cor do texto, ou
/// ligado com cor própria.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decoration {
    Off,
    On(Option<Color>),
}

macro_rules! merge_fields {
    ($self:ident, $other:ident, $($field:ident),+ $(,)?) => {
        $(if let Some(v) = $other.$field.clone() { $self.$field = Some(v); })+
    };
}

impl StyleProps {
    /// Converte TODAS as unidades de viewport (vw/vh/vmin/vmax) em píxeis
    /// lógicos contra a janela dada, in-place.
    ///
    /// Porquê aqui e não no layout: o caminho de layout da Bevy 0.19 resolve
    /// `VMin` com um tamanho de alvo que não acompanha a janela em runtime
    /// (medido: numa 600×1200 o `vmin` computava com 1200) — e o pipeline de
    /// texto nem chega a receber a unidade. Resolvendo no meu lado, com o
    /// viewport que JÁ uso para `@media`, as unidades significam o mesmo em
    /// todo o lado. Píxeis resultantes seguem o caminho normal (o scale
    /// factor do taffy aplica-se-lhes como a qualquer px autoral).
    pub fn resolve_viewport(&mut self, viewport: (f32, f32)) {
        let (vw, vh) = (viewport.0.max(1.0), viewport.1.max(1.0));
        let vmin = vw.min(vh);
        let vmax = vw.max(vh);
        let to_px = |val: Val| -> Val {
            match val {
                Val::Vw(v) => Val::Px(v / 100.0 * vw),
                Val::Vh(v) => Val::Px(v / 100.0 * vh),
                Val::VMin(v) => Val::Px(v / 100.0 * vmin),
                Val::VMax(v) => Val::Px(v / 100.0 * vmax),
                other => other,
            }
        };
        let rect = |r: &mut UiRect| {
            r.top = to_px(r.top);
            r.right = to_px(r.right);
            r.bottom = to_px(r.bottom);
            r.left = to_px(r.left);
        };
        for val in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.top,
            &mut self.right,
            &mut self.bottom,
            &mut self.left,
            &mut self.row_gap,
            &mut self.column_gap,
            &mut self.font_size,
        ] {
            // Expressões fecham AQUI, contra o mesmo viewport autoral das
            // media queries — e saem como px simples para o resto do pipeline.
            *val = val.take().and_then(|measure| match measure {
                Measure::Plain(v) => Some(Measure::Plain(to_px(v))),
                Measure::Expr(expr) => expr.eval(viewport).map(|px| Measure::Plain(Val::Px(px))),
            });
        }
        if let Some(r) = &mut self.padding {
            rect(r);
        }
        if let Some(r) = &mut self.margin {
            rect(r);
        }
        if let Some(r) = &mut self.border {
            rect(r);
        }
        if let Some(r) = &mut self.border_radius {
            r.top_left = to_px(r.top_left);
            r.top_right = to_px(r.top_right);
            r.bottom_right = to_px(r.bottom_right);
            r.bottom_left = to_px(r.bottom_left);
        }
        if let Some(shadows) = &mut self.box_shadow {
            for shadow in shadows.iter_mut() {
                shadow.x_offset = to_px(shadow.x_offset);
                shadow.y_offset = to_px(shadow.y_offset);
                shadow.blur_radius = to_px(shadow.blur_radius);
                shadow.spread_radius = to_px(shadow.spread_radius);
            }
        }
    }

    /// Overlays `other` on top of `self` — every field `other` sets wins.
    pub fn merge(&mut self, other: &StyleProps) {
        merge_fields!(
            self,
            other,
            display,
            position,
            flex_direction,
            flex_wrap,
            flex_grow,
            flex_shrink,
            align_items,
            align_self,
            justify_content,
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            top,
            right,
            bottom,
            left,
            padding,
            margin,
            border,
            row_gap,
            column_gap,
            aspect_ratio,
            overflow_clip,
            grid_template_columns,
            grid_template_rows,
            grid_auto_flow,
            grid_column,
            grid_row,
            background,
            border_color,
            border_radius,
            outline,
            box_shadow,
            opacity,
            z_index,
            rotate,
            scale,
            translate,
            cursor,
            pointer_none,
            color,
            font_size,
            font_size_em,
            font_size_rem,
            font_weight,
            text_align,
            linebreak,
            text_underline,
            text_strikethrough,
            text_shadow,
            image_tint,
            image_mode,
        );
    }

    /// Preenche os campos que o CSS HERDA com o valor computado do pai —
    /// só onde a própria cascata não disse nada (herdado perde para folha e
    /// inline). É o set web: tudo o que descreve o TEXTO. Um painel não tem
    /// cor própria; os seus textos sim, e por isso a cor desce a árvore.
    pub fn apply_inherited(&mut self, base: &StyleProps) {
        macro_rules! inherit {
            ($($field:ident),+ $(,)?) => {
                $(if self.$field.is_none() { self.$field = base.$field.clone(); })+
            };
        }
        inherit!(
            color,
            font_size,
            font_weight,
            text_align,
            linebreak,
            text_shadow
        );
    }

    /// Escreve o estilo de layout em `node`, FROM SCRATCH.
    ///
    /// É isto que torna `@media` honesto: campos que a cascata NÃO menciona
    /// nesta passagem voltam ao omissão — sem isto, um bloco de retrato que
    /// fixou `left` deixava o valor preso para sempre (o paisagem nunca o
    /// "desfazia", só falhava em o redefinir). Os campos são os mesmos que
    /// [`Self::apply_to_node`] escreve; o resto do `Node` (scroll, focus,
    /// tamanhos computados) é de outros sistemas e fica intacto.
    pub fn apply_fresh(&self, node: &mut Node) {
        let clean = Node::default();
        node.display = clean.display;
        node.position_type = clean.position_type;
        node.flex_direction = clean.flex_direction;
        node.flex_wrap = clean.flex_wrap;
        node.flex_grow = clean.flex_grow;
        node.flex_shrink = clean.flex_shrink;
        node.align_items = clean.align_items;
        node.align_self = clean.align_self;
        node.justify_content = clean.justify_content;
        node.width = clean.width;
        node.height = clean.height;
        node.min_width = clean.min_width;
        node.min_height = clean.min_height;
        node.max_width = clean.max_width;
        node.max_height = clean.max_height;
        node.top = clean.top;
        node.right = clean.right;
        node.bottom = clean.bottom;
        node.left = clean.left;
        node.padding = clean.padding;
        node.margin = clean.margin;
        node.border = clean.border;
        node.row_gap = clean.row_gap;
        node.column_gap = clean.column_gap;
        node.border_radius = clean.border_radius;
        node.aspect_ratio = clean.aspect_ratio;
        node.overflow = clean.overflow;
        node.grid_template_columns = clean.grid_template_columns.clone();
        node.grid_template_rows = clean.grid_template_rows.clone();
        node.grid_auto_flow = clean.grid_auto_flow;
        node.grid_column = clean.grid_column;
        node.grid_row = clean.grid_row;
        self.apply_to_node(node);
    }

    /// Writes the layout half of the style into a `Node`.
    pub fn apply_to_node(&self, node: &mut Node) {
        macro_rules! set {
            ($($field:ident),+ $(,)?) => {
                $(if let Some(v) = self.$field { node.$field = v; })+
            };
        }
        set!(
            display,
            flex_direction,
            flex_wrap,
            flex_grow,
            flex_shrink,
            align_items,
            align_self,
            justify_content,
            padding,
            margin,
            border,
            border_radius,
            grid_auto_flow,
            grid_column,
            grid_row,
        );
        // Medidas escalares podem ser Measure::Expr pré-resolução — pós-
        // resolução é sempre Plain, e `to_val` é o extrator honesto.
        macro_rules! set_measure {
            ($($field:ident),+ $(,)?) => {
                $(if let Some(v) = &self.$field { node.$field = v.to_val(); })+
            };
        }
        set_measure!(
            width, height, min_width, min_height, max_width, max_height, top, right, bottom, left,
            row_gap, column_gap,
        );
        // As pistas de grelha são Vec (não Copy): clone fora do macro.
        if let Some(tracks) = &self.grid_template_columns {
            node.grid_template_columns = tracks.clone();
        }
        if let Some(tracks) = &self.grid_template_rows {
            node.grid_template_rows = tracks.clone();
        }
        // `position` and `overflow` do not share their CSS name with `Node`.
        if let Some(v) = self.position {
            node.position_type = v;
        }
        if let Some(v) = self.aspect_ratio {
            node.aspect_ratio = Some(v);
        }
        if let Some(clip) = self.overflow_clip {
            node.overflow = if clip {
                Overflow::clip()
            } else {
                Overflow::visible()
            };
        }
    }
}

/// A parsed stylesheet: rules kept in source order.
///
/// Cada chamada a [`StyleSheet::parse_into`] cria uma FOLHA nova (índice
/// sequencial) — é a unidade de *scoping*: uma `<UiRoot>` só vê as folhas que
/// lhe foram atribuídas no build (ver `UiRootSheets`), como uma shadow-DOM
/// vê só os seus estilos. Sem isso, uma classe comum a duas folhas (`.empty`
/// do menu vs. um slot do HUD) vaza de uma para a outra.
#[derive(Debug, Clone, Default, Resource)]
pub struct StyleSheet {
    pub rules: Vec<Rule>,
    /// Próximo índice de folha a atribuir (`parse_into`).
    next_sheet: usize,
}

impl StyleSheet {
    /// Parses a stylesheet, appending its rules after any already present.
    ///
    /// Devolve o índice da folha criada — é ele que vai no `UiRootSheets` da
    /// raiz que a declarou. Unknown properties and malformed rules are
    /// reported and skipped: a typo in one colour must not take the whole HUD
    /// down. `@media (…) { … }` blocks nest their rules under a [`MediaCond`];
    /// nothing else nests.
    pub fn parse_into(&mut self, source: &str) -> usize {
        let sheet = self.next_sheet;
        self.next_sheet += 1;
        let text = strip_comments(source);
        self.parse_rules(&text, None, sheet);
        sheet
    }

    fn parse_rules(&mut self, text: &str, media: Option<MediaCond>, sheet: usize) {
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            let head = rest[..open].trim();
            let after = &rest[open + 1..];
            let Some(close) = find_matching_brace(after) else {
                warn!("ui style: unclosed rule for `{head}`");
                return;
            };
            let body = &after[..close];
            rest = &after[close + 1..];
            if let Some(cond_text) = head.strip_prefix("@media") {
                match parse_media_cond(cond_text) {
                    Some(cond) => {
                        // As condições EMPILHAM: o bloco interno herda as
                        // exigências do externo (preenchendo os seus vazios).
                        let merged = merge_media(media.clone(), cond);
                        self.parse_rules(body, merged, sheet);
                    }
                    None => warn!("ui style: @media ilegível `{cond_text}` — bloco saltado"),
                }
                continue;
            }
            let props = parse_declarations(body, head);
            for selector_text in head.split(',') {
                let selector_text = selector_text.trim();
                if selector_text.is_empty() {
                    continue;
                }
                match parse_selector(selector_text) {
                    Some(selector) => {
                        let order = self.rules.len();
                        self.rules.push(Rule {
                            selector,
                            props: props.clone(),
                            order,
                            media: media.clone(),
                            sheet,
                        });
                    }
                    None => warn!("ui style: unparsable selector `{selector_text}` — skipped"),
                }
            }
        }
    }

    /// Cascade for one element: every matching rule merged by specificity, then
    /// source order. Rules under a failed `@media` condition are ignored.
    ///
    /// `chain` is the ancestor path, root first, with the element itself last;
    /// `viewport` is the AUTHORED-space window (`scale::ui_viewport`) — media
    /// queries, viewport units and authored px all speak that one language.
    /// `sheets` restringe as folhas visíveis (as carregadas pela `UiRoot` do
    /// ramo); `None` = todas — é o que os testes e o `resolve_one` usam.
    pub fn resolve(
        &self,
        chain: &[ElementRef<'_>],
        viewport: (f32, f32),
        sheets: Option<&[usize]>,
    ) -> StyleProps {
        let mut matched: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| sheets.is_none_or(|s| s.contains(&r.sheet)))
            .filter(|r| {
                r.media
                    .as_ref()
                    .is_none_or(|cond| cond.matches(viewport.0, viewport.1))
            })
            .filter(|r| r.selector.matches(chain))
            .collect();
        matched.sort_by_key(|r| (r.selector.specificity(), r.order));
        let mut out = StyleProps::default();
        for rule in matched {
            out.merge(&rule.props);
        }
        out
    }

    /// Convenience for a standalone element with no styled ancestors.
    pub fn resolve_one(
        &self,
        tag: &str,
        id: Option<&str>,
        classes: &[String],
        state: StyleState,
        viewport: (f32, f32),
    ) -> StyleProps {
        self.resolve(
            &[ElementRef {
                tag,
                id,
                classes,
                state,
            }],
            viewport,
            None,
        )
    }
}

/// Index of the `)` that closes a `(` before `text` (nesting-aware) — o que
/// fecha funções e grupos dentro de `calc(…)`.
fn find_matching_paren(text: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Index of the `}` that closes the block opened before `text` (nesting-aware).
fn find_matching_brace(text: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// `AND` de duas condições de media: as exigências do filho preenchem-se
/// sobre as do pai — uma regra num bloco aninhado só vale quando AMBOS valem.
fn merge_media(parent: Option<MediaCond>, child: MediaCond) -> Option<MediaCond> {
    let Some(parent) = parent else {
        return Some(child);
    };
    let fill = |parent: Option<f32>, child: Option<f32>, max: bool| -> Option<f32> {
        match (parent, child) {
            (Some(p), Some(c)) => {
                // Dois máximos = o mais apertado; dois mínimos = o mais alto.
                if max { Some(p.min(c)) } else { Some(p.max(c)) }
            }
            (Some(p), None) => Some(p),
            (None, None) => None,
            (_, Some(c)) => Some(c),
        }
    };
    Some(MediaCond {
        min_width: fill(parent.min_width, child.min_width, false),
        max_width: fill(parent.max_width, child.max_width, true),
        min_height: fill(parent.min_height, child.min_height, false),
        max_height: fill(parent.max_height, child.max_height, true),
        min_aspect: fill(parent.min_aspect, child.min_aspect, false),
        max_aspect: fill(parent.max_aspect, child.max_aspect, true),
        portrait: parent.portrait || child.portrait,
        landscape: parent.landscape || child.landscape,
    })
}

/// Removes `/* … */` comments so they can appear anywhere.
fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// `.card .fill:hover` → two compounds, the second scoped to hover.
pub fn parse_selector(text: &str) -> Option<Selector> {
    let parts: Vec<Compound> = text
        .split_whitespace()
        .map(parse_compound)
        .collect::<Option<Vec<_>>>()?;
    if parts.is_empty() {
        return None;
    }
    Some(Selector { parts })
}

/// One whitespace-free chunk: `UiPanel.card#hero:hover`.
fn parse_compound(text: &str) -> Option<Compound> {
    let mut compound = Compound::default();
    // The state pseudo is always last, so peel it off before the rest.
    let head = match text.split_once(':') {
        Some((head, pseudo)) => {
            compound.state = match pseudo {
                "hover" => StyleState::Hover,
                "active" | "pressed" => StyleState::Active,
                "disabled" => StyleState::Disabled,
                other => {
                    warn!("ui style: unknown pseudo-class `:{other}` — treated as normal");
                    StyleState::Normal
                }
            };
            head
        }
        None => text,
    };
    if head.is_empty() || head == "*" {
        return Some(compound);
    }
    // Split on the `.`/`#` boundaries, keeping the marker with its name.
    let mut token = String::new();
    let mut marker = ' ';
    let flush = |marker: char, token: &mut String, compound: &mut Compound| -> bool {
        if token.is_empty() {
            return marker == ' ';
        }
        match marker {
            '.' => compound.classes.push(token.to_ascii_lowercase()),
            '#' => {
                if compound.id.is_some() {
                    return false;
                }
                compound.id = Some(std::mem::take(token));
            }
            _ => compound.tag = Some(token.to_ascii_lowercase()),
        }
        token.clear();
        true
    };
    for c in head.chars() {
        if c == '.' || c == '#' {
            if !flush(marker, &mut token, &mut compound) {
                return None;
            }
            marker = c;
            continue;
        }
        if !(c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return None;
        }
        token.push(c);
    }
    if !flush(marker, &mut token, &mut compound) {
        return None;
    }
    // A bare `.` or `#` names nothing.
    if compound.tag.is_none()
        && compound.id.is_none()
        && compound.classes.is_empty()
        && compound.state == StyleState::Normal
    {
        return None;
    }
    Some(compound)
}

/// Parses a `prop: value; prop: value` body. `origin` only names the rule in
/// warnings.
pub fn parse_declarations(body: &str, origin: &str) -> StyleProps {
    let mut props = StyleProps::default();
    for decl in body.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let Some((name, value)) = decl.split_once(':') else {
            warn!("ui style: `{decl}` in `{origin}` is not `prop: value` — skipped");
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if !apply_declaration(&mut props, &name, value) {
            warn!("ui style: unknown property `{name}: {value}` in `{origin}` — skipped");
        }
    }
    props
}

/// Parsers "com aviso": um nome de propriedade CONHECIDO com um valor ilegível
/// (`width: wide`) não pode cair em silêncio — o autor nunca encontra o erro.
fn parse_measure_in(value: &str, prop: &str) -> Option<Measure> {
    let parsed = parse_measure(value);
    if parsed.is_none() {
        warn!("ui style: valor `{value}` ilegível para `{prop}` — declaração saltada");
    }
    parsed
}

fn parse_rect_in(value: &str, prop: &str) -> Option<UiRect> {
    let parsed = parse_rect(value);
    if parsed.is_none() {
        warn!("ui style: valor `{value}` ilegível para `{prop}` — declaração saltada");
    }
    parsed
}

fn parse_color_in(value: &str, prop: &str) -> Option<Color> {
    let parsed = parse_color(value);
    if parsed.is_none() {
        warn!("ui style: cor `{value}` ilegível para `{prop}` — declaração saltada");
    }
    parsed
}

fn parse_number_in(value: &str, prop: &str) -> Option<f32> {
    let parsed = parse_number(value);
    if parsed.is_none() {
        warn!("ui style: número `{value}` ilegível para `{prop}` — declaração saltada");
    }
    parsed
}

/// Sets one property; `false` when the name or the value is not understood.
fn apply_declaration(props: &mut StyleProps, name: &str, value: &str) -> bool {
    match name {
        // layout
        "display" => props.display = parse_display(value),
        "position" => props.position = parse_position(value),
        "direction" | "flex-direction" => props.flex_direction = parse_direction(value),
        "wrap" | "flex-wrap" => {
            props.flex_wrap = Some(match value {
                "wrap" => FlexWrap::Wrap,
                "wrap-reverse" => FlexWrap::WrapReverse,
                _ => FlexWrap::NoWrap,
            })
        }
        "grow" | "flex-grow" => props.flex_grow = value.parse().ok(),
        "shrink" | "flex-shrink" => props.flex_shrink = value.parse().ok(),
        "align" | "align-items" => props.align_items = parse_align_items(value),
        "align-self" => props.align_self = parse_align_self(value),
        "justify" | "justify-content" => props.justify_content = parse_justify(value),
        "width" => props.width = parse_measure_in(value, name),
        "height" => props.height = parse_measure_in(value, name),
        "min-width" => props.min_width = parse_measure_in(value, name),
        "min-height" => props.min_height = parse_measure_in(value, name),
        "max-width" => props.max_width = parse_measure_in(value, name),
        "max-height" => props.max_height = parse_measure_in(value, name),
        "top" => props.top = parse_measure_in(value, name),
        "right" => props.right = parse_measure_in(value, name),
        "bottom" => props.bottom = parse_measure_in(value, name),
        "left" => props.left = parse_measure_in(value, name),
        "padding" => props.padding = parse_rect_in(value, name),
        "margin" => props.margin = parse_rect_in(value, name),
        "border-width" => props.border = parse_rect_in(value, name),
        "gap" => {
            let gap = parse_measure_in(value, name);
            props.row_gap = gap.clone();
            props.column_gap = gap;
        }
        "row-gap" => props.row_gap = parse_measure_in(value, name),
        "column-gap" => props.column_gap = parse_measure_in(value, name),
        "aspect" | "aspect-ratio" => props.aspect_ratio = parse_number_in(value, name),
        "overflow" => props.overflow_clip = Some(value == "clip" || value == "hidden"),
        // grid
        "grid-template-columns" | "grid-cols" => props.grid_template_columns = parse_tracks(value),
        "grid-template-rows" | "grid-rows" => props.grid_template_rows = parse_tracks(value),
        "grid-auto-flow" | "flow" => props.grid_auto_flow = parse_auto_flow(value),
        "grid-column" => props.grid_column = parse_placement(value),
        "grid-row" => props.grid_row = parse_placement(value),
        // paint
        "background" | "background-color" => props.background = parse_color_in(value, name),
        "border-color" => props.border_color = parse_color_in(value, name),
        // `border: 1.5 #aabbcc` — width and colour together, like CSS shorthand.
        "border" => return parse_border_shorthand(props, value),
        "radius" | "border-radius" => props.border_radius = parse_radius(value),
        "outline" => props.outline = parse_outline(value),
        "box-shadow" => props.box_shadow = parse_box_shadows(value),
        "opacity" => props.opacity = parse_number_in(value, name),
        "z" | "z-index" => props.z_index = value.parse().ok(),
        "rotate" => props.rotate = parse_number_in(value.trim_end_matches("deg"), name),
        "scale" => props.scale = parse_number_in(value, name),
        "translate" => {
            let numbers: Vec<f32> = value.split_whitespace().filter_map(parse_number).collect();
            props.translate = match numbers.len() {
                0 => None,
                1 => Some(Vec2::splat(numbers[0])),
                _ => Some(Vec2::new(numbers[0], numbers[1])),
            };
        }
        "cursor" => props.cursor = parse_cursor(value),
        "pointer-events" => props.pointer_none = Some(value.eq_ignore_ascii_case("none")),
        // text
        "color" => props.color = parse_color_in(value, name),
        "font-size" => {
            // Unidades RELATIVAS (`em`, `rem`, `%`) dependem da fonte herdada
            // e só podem ser resolvidas no momento de aplicar — chegam como
            // campos próprios e o runtime fecha-as contra a base do pai.
            // "rem" tem de ser testado ANTES de "em" (termina em "em").
            let value = value.trim();
            if let Some(n) = value
                .strip_suffix("rem")
                .and_then(|n| n.trim().parse().ok())
            {
                props.font_size_rem = Some(n);
            } else if let Some(n) = value.strip_suffix("em").and_then(|n| n.trim().parse().ok()) {
                props.font_size_em = Some(n);
            } else {
                props.font_size = parse_measure_in(value, name);
            }
        }
        "font-weight" | "weight" => props.font_weight = parse_font_weight(value),
        "text-decoration" => {
            // `underline`, `line-through [#cor]`, `none` — dois traços de uma vez.
            let mut underline = Decoration::Off;
            let mut strike = Decoration::Off;
            let mut color = None;
            for token in value.split_whitespace() {
                match token {
                    "underline" => underline = Decoration::On(color.take()),
                    "line-through" | "strikethrough" => strike = Decoration::On(color.take()),
                    "none" => {
                        underline = Decoration::Off;
                        strike = Decoration::Off;
                    }
                    other => color = parse_color(other).or(color),
                }
            }
            if underline != Decoration::Off {
                props.text_underline = Some(underline);
            }
            if strike != Decoration::Off {
                props.text_strikethrough = Some(strike);
            }
        }
        "underline" => props.text_underline = Some(parse_decoration(value)),
        "strikethrough" | "line-through" => {
            props.text_strikethrough = Some(parse_decoration(value))
        }
        "line-break" => props.linebreak = Some(parse_linebreak(value)),
        "text-shadow" => props.text_shadow = Some(parse_text_shadow(value)),
        "text-align" => {
            props.text_align = Some(match value {
                "center" => bevy::text::Justify::Center,
                "right" => bevy::text::Justify::Right,
                "justify" => bevy::text::Justify::Justified,
                _ => bevy::text::Justify::Left,
            })
        }
        // image
        "tint" | "image-tint" => props.image_tint = parse_color(value),
        "image-mode" | "fit" => {
            props.image_mode = Some(match value {
                "stretch" => NodeImageMode::Stretch,
                _ => NodeImageMode::Auto,
            })
        }
        _ => return false,
    }
    true
}

/// A parsed `text-shadow`: offset in logical px plus colour.
///
/// The HUD has to stay legible over a white desert and a black forest at the
/// same time, and a 1 px dark offset does that for a fraction of the cost of an
/// outlined font.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextShadowSpec {
    pub offset: Vec2,
    pub color: Color,
}

/// `outline: <width> <colour>` — desenhado FORA da borda pelo bevy_ui
/// (`Outline`), é o rim que separa a forma do fundo sem engrossar a
/// própria borda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineSpec {
    pub width: f32,
    pub color: Color,
}

/// `<width> <colour>` — `1 #2b1d10aa`; `none` desliga (width 0).
fn parse_outline(value: &str) -> Option<OutlineSpec> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(OutlineSpec {
            width: 0.0,
            color: Color::NONE,
        });
    }
    let mut width = 1.0_f32;
    let mut color = Color::srgba(0.07, 0.05, 0.03, 0.7);
    let mut seen = false;
    for token in value.split_whitespace() {
        if let Some(parsed) = parse_color(token) {
            color = parsed;
            seen = true;
        } else if let Ok(number) = token.parse::<f32>() {
            width = number.max(0.0);
            seen = true;
        }
    }
    seen.then_some(OutlineSpec { width, color })
}

/// `2 2 #000000cc`, `1 1` (default colour), or `none`.
fn parse_text_shadow(value: &str) -> Option<TextShadowSpec> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") || value == "0" {
        return None;
    }
    let mut offset = Vec2::splat(1.0);
    let mut color = Color::srgba(0.0, 0.0, 0.0, 0.75);
    let mut numbers: Vec<f32> = Vec::new();
    for token in value.split_whitespace() {
        if let Some(parsed) = parse_color(token) {
            color = parsed;
        } else if let Ok(number) = token.parse::<f32>() {
            numbers.push(number);
        }
    }
    match numbers.len() {
        0 => {}
        1 => offset = Vec2::splat(numbers[0]),
        _ => offset = Vec2::new(numbers[0], numbers[1]),
    }
    Some(TextShadowSpec { offset, color })
}

/// Lista de sombras separadas por vírgulas; `none` devolve a lista vazia
/// (explicitamente sem sombra, o que remove uma herdada).
///
/// Cada sombra segue a ordem CSS `x y blur spread color` — os números são
/// opcionais a partir do terceiro e a cor por omissão é um preto a 40 %.
fn parse_box_shadows(value: &str) -> Option<Vec<ShadowStyle>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let shadows: Vec<ShadowStyle> = value
        .split(',')
        .filter_map(|spec| parse_box_shadow(spec.trim()))
        .collect();
    Some(shadows)
}

/// Uma sombra: `4 8 12 #000000aa` (x, y, blur, spread?, cor?).
fn parse_box_shadow(spec: &str) -> Option<ShadowStyle> {
    if spec.is_empty() {
        return None;
    }
    let mut x = Val::Px(0.0);
    let mut y = Val::Px(0.0);
    let mut blur = Val::Px(0.0);
    let mut spread = Val::Px(0.0);
    let mut color = Color::srgba(0.0, 0.0, 0.0, 0.4);
    let mut numbers = 0;
    let mut seen = false;
    for token in spec.split_whitespace() {
        if let Some(parsed) = parse_color(token) {
            color = parsed;
            seen = true;
        } else if let Some(val) = parse_val(token) {
            match numbers {
                0 => x = val,
                1 => y = val,
                2 => blur = val,
                _ => spread = val,
            }
            numbers += 1;
            seen = true;
        }
    }
    seen.then_some(ShadowStyle {
        color,
        x_offset: x,
        y_offset: y,
        spread_radius: spread,
        blur_radius: blur,
    })
}

/// Espessura do tipo: `bold`/`normal`/`light`/`semibold`/`black` ou 100–1000.
fn parse_font_weight(value: &str) -> Option<f32> {
    match value.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(100.0),
        "light" => Some(300.0),
        "normal" | "" => Some(400.0),
        "medium" => Some(500.0),
        "semibold" => Some(600.0),
        "bold" => Some(700.0),
        "black" => Some(900.0),
        other => other
            .parse::<f32>()
            .ok()
            .filter(|w| (1.0..=1000.0).contains(w)),
    }
}

/// `underline [#cor]` / `none`; `true` liga com a cor do texto.
fn parse_decoration(value: &str) -> Decoration {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("false") {
        return Decoration::Off;
    }
    let mut color = None;
    for token in value.split_whitespace() {
        if let Some(parsed) = parse_color(token) {
            color = Some(parsed);
        }
    }
    Decoration::On(color)
}

/// `word` (omissão), `char`, `word-char`, `none`/`nowrap`.
fn parse_linebreak(value: &str) -> LineBreak {
    match value.trim().to_ascii_lowercase().as_str() {
        "char" | "any" => LineBreak::AnyCharacter,
        "word-char" | "word-or-char" => LineBreak::WordOrCharacter,
        "none" | "nowrap" | "no-wrap" => LineBreak::NoWrap,
        _ => LineBreak::WordBoundary,
    }
}

/// Lista de pistas de grelha: `64 1fr 25% auto repeat(4, 1fr)`.
///
/// O tokenizer respeita parêntesis — `repeat(4, 1fr)` traz um espaço DENTRO
/// do token e um `split_whitespace` partia-o ao meio.
pub fn parse_tracks(value: &str) -> Option<Vec<RepeatedGridTrack>> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for ch in value.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    let tracks: Vec<RepeatedGridTrack> = tokens
        .into_iter()
        .filter_map(|token| parse_track_token(&token))
        .collect();
    (!tracks.is_empty()).then_some(tracks)
}

/// Uma pista: simples ou `repeat(n, pista)`.
fn parse_track_token(token: &str) -> Option<RepeatedGridTrack> {
    if let Some(rest) = token
        .strip_prefix("repeat(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let (count, track) = rest.split_once(',')?;
        let count: u16 = count.trim().parse().ok()?;
        let track = track.trim();
        if let Some(fr) = track.strip_suffix("fr") {
            return Some(RepeatedGridTrack::fr(count, fr.trim().parse().ok()?));
        }
        if let Some(pct) = track.strip_suffix('%') {
            return Some(RepeatedGridTrack::percent(
                GridTrackRepetition::Count(count),
                pct.trim().parse().ok()?,
            ));
        }
        return match track {
            "auto" => Some(RepeatedGridTrack::auto(count)),
            "min-content" => Some(RepeatedGridTrack::min_content(count)),
            "max-content" => Some(RepeatedGridTrack::max_content(count)),
            plain => plain
                .parse::<f32>()
                .ok()
                .map(|value| RepeatedGridTrack::px(GridTrackRepetition::Count(count), value)),
        };
    }
    parse_single_track(token).map(RepeatedGridTrack::from)
}

/// Pista simples: `64` px, `25%`, `2fr`, `auto`, `min-content`, `max-content`.
fn parse_single_track(token: &str) -> Option<GridTrack> {
    if let Some(fr) = token.strip_suffix("fr") {
        return fr.trim().parse::<f32>().ok().map(GridTrack::fr);
    }
    if let Some(pct) = token.strip_suffix('%') {
        return pct.trim().parse::<f32>().ok().map(GridTrack::percent);
    }
    match token.trim() {
        "auto" => Some(GridTrack::auto()),
        "min-content" => Some(GridTrack::min_content()),
        "max-content" => Some(GridTrack::max_content()),
        trimmed => trimmed.parse::<f32>().ok().map(GridTrack::px),
    }
}

/// `row` (omissão), `column`, `row dense`, `column dense`.
pub fn parse_auto_flow(value: &str) -> Option<GridAutoFlow> {
    let mut column = false;
    let mut dense = false;
    for token in value.split_whitespace() {
        match token {
            "column" => column = true,
            "dense" => dense = true,
            "row" => {}
            _ => return None,
        }
    }
    Some(match (column, dense) {
        (true, true) => GridAutoFlow::ColumnDense,
        (true, false) => GridAutoFlow::Column,
        (false, true) => GridAutoFlow::RowDense,
        (false, false) => GridAutoFlow::Row,
    })
}

/// Colocação de item de grelha: `auto`, `2`, `span 3`, `1 / 4`, `2 / span 3`.
fn parse_placement(value: &str) -> Option<GridPlacement> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") || value.is_empty() {
        return Some(GridPlacement::default());
    }
    let parse_line =
        |text: &str| -> Option<i16> { text.parse::<i16>().ok().filter(|line| *line != 0) };
    let parse_span = |text: &str| -> Option<u16> { text.strip_prefix("span ")?.parse().ok() };
    match value.split_once('/') {
        None => match parse_line(value) {
            // `2` — começa na linha 2.
            Some(line) => Some(GridPlacement::start(line)),
            // `span 3` sozinho — o início é automático.
            None => parse_span(value).map(GridPlacement::span),
        },
        Some((start_text, end_text)) => {
            let start = parse_line(start_text.trim())?;
            let end_text = end_text.trim();
            // `2 / span 3`, ou `1 / 4` (fim exclusivo; negativo conta de trás).
            if let Some(span) = parse_span(end_text) {
                Some(GridPlacement::start_span(start, span))
            } else {
                Some(GridPlacement::start_end(start, parse_line(end_text)?))
            }
        }
    }
}

/// Cursor do rato sobre o elemento.
fn parse_cursor(value: &str) -> Option<CursorIcon> {
    let icon = match value.trim().to_ascii_lowercase().as_str() {
        "default" => SystemCursorIcon::Default,
        "pointer" | "hand" => SystemCursorIcon::Pointer,
        "text" => SystemCursorIcon::Text,
        "crosshair" => SystemCursorIcon::Crosshair,
        "move" => SystemCursorIcon::Move,
        "wait" => SystemCursorIcon::Wait,
        "progress" => SystemCursorIcon::Progress,
        "help" => SystemCursorIcon::Help,
        "not-allowed" => SystemCursorIcon::NotAllowed,
        "grab" => SystemCursorIcon::Grab,
        "grabbing" => SystemCursorIcon::Grabbing,
        "cell" => SystemCursorIcon::Cell,
        "col-resize" => SystemCursorIcon::ColResize,
        "row-resize" => SystemCursorIcon::RowResize,
        "ew-resize" => SystemCursorIcon::EwResize,
        "ns-resize" => SystemCursorIcon::NsResize,
        "all-scroll" => SystemCursorIcon::AllScroll,
        _ => return None,
    };
    Some(CursorIcon::from(icon))
}

fn parse_border_shorthand(props: &mut StyleProps, value: &str) -> bool {
    let mut width: Option<UiRect> = None;
    let mut color: Option<Color> = None;
    for token in value.split_whitespace() {
        if let Some(c) = parse_color(token) {
            color = Some(c);
        } else if let Some(rect) = parse_rect(token) {
            width = Some(rect);
        } else {
            return false;
        }
    }
    if width.is_none() && color.is_none() {
        return false;
    }
    props.border = width.or(props.border);
    props.border_color = color.or(props.border_color);
    true
}

fn parse_display(value: &str) -> Option<Display> {
    Some(match value {
        "none" => Display::None,
        "block" => Display::Block,
        "grid" => Display::Grid,
        _ => Display::Flex,
    })
}

fn parse_position(value: &str) -> Option<PositionType> {
    Some(match value {
        "absolute" | "fixed" => PositionType::Absolute,
        _ => PositionType::Relative,
    })
}

fn parse_direction(value: &str) -> Option<FlexDirection> {
    Some(match value {
        "column" => FlexDirection::Column,
        "row-reverse" => FlexDirection::RowReverse,
        "column-reverse" => FlexDirection::ColumnReverse,
        _ => FlexDirection::Row,
    })
}

fn parse_align_items(value: &str) -> Option<AlignItems> {
    Some(match value {
        "start" | "flex-start" => AlignItems::FlexStart,
        "end" | "flex-end" => AlignItems::FlexEnd,
        "center" => AlignItems::Center,
        "baseline" => AlignItems::Baseline,
        "stretch" => AlignItems::Stretch,
        _ => return None,
    })
}

fn parse_align_self(value: &str) -> Option<AlignSelf> {
    Some(match value {
        "start" | "flex-start" => AlignSelf::FlexStart,
        "end" | "flex-end" => AlignSelf::FlexEnd,
        "center" => AlignSelf::Center,
        "baseline" => AlignSelf::Baseline,
        "stretch" => AlignSelf::Stretch,
        _ => return None,
    })
}

fn parse_justify(value: &str) -> Option<JustifyContent> {
    Some(match value {
        "start" | "flex-start" => JustifyContent::FlexStart,
        "end" | "flex-end" => JustifyContent::FlexEnd,
        "center" => JustifyContent::Center,
        "space-between" => JustifyContent::SpaceBetween,
        "space-around" => JustifyContent::SpaceAround,
        "space-evenly" => JustifyContent::SpaceEvenly,
        _ => return None,
    })
}

/// Bare numbers are pixels — the unit a HUD author actually thinks in.
///
/// Viewport units make a layout **responsive**: `40vw` = 40 % da largura da
/// janela, `6vh` da altura, `vmin`/`vmax` do menor/maior dos dois. A janela
/// pode ter qualquer proporção — o valor acompanha.
pub fn parse_val(value: &str) -> Option<Val> {
    let value = value.trim();
    if value == "auto" {
        return Some(Val::Auto);
    }
    for (suffix, make) in [
        ("vmin", Val::VMin as fn(f32) -> Val),
        ("vmax", Val::VMax as fn(f32) -> Val),
        ("vw", Val::Vw as fn(f32) -> Val),
        ("vh", Val::Vh as fn(f32) -> Val),
    ] {
        if let Some(number) = value.strip_suffix(suffix) {
            return number.trim().parse().ok().map(make);
        }
    }
    if let Some(pct) = value.strip_suffix('%') {
        return pct.trim().parse().ok().map(Val::Percent);
    }
    let number = value.strip_suffix("px").unwrap_or(value).trim();
    number.parse().ok().map(Val::Px)
}

fn parse_number(value: &str) -> Option<f32> {
    value
        .trim()
        .strip_suffix("px")
        .unwrap_or(value)
        .trim()
        .parse()
        .ok()
}

/// CSS box shorthand: 1 value (all), 2 (vertical horizontal), 3 (top h bottom)
/// or 4 (top right bottom left).
pub fn parse_rect(value: &str) -> Option<UiRect> {
    let parts: Vec<Val> = value
        .split_whitespace()
        .map(parse_val)
        .collect::<Option<Vec<_>>>()?;
    Some(match parts.len() {
        1 => UiRect::all(parts[0]),
        2 => UiRect::axes(parts[1], parts[0]),
        3 => UiRect {
            top: parts[0],
            right: parts[1],
            bottom: parts[2],
            left: parts[1],
        },
        4 => UiRect {
            top: parts[0],
            right: parts[1],
            bottom: parts[2],
            left: parts[3],
        },
        _ => return None,
    })
}

fn parse_radius(value: &str) -> Option<BorderRadius> {
    let parts: Vec<Val> = value
        .split_whitespace()
        .map(parse_val)
        .collect::<Option<Vec<_>>>()?;
    Some(match parts.len() {
        1 => BorderRadius::all(parts[0]),
        4 => BorderRadius {
            top_left: parts[0],
            top_right: parts[1],
            bottom_right: parts[2],
            bottom_left: parts[3],
        },
        _ => return None,
    })
}

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgba(r,g,b,a)` (0-255 / 0-1 alpha),
/// `none`/`transparent`, os nomes do HUD — e a paleta Tailwind por nome
/// (`slate-900`, `rose-400/80`, `amber`), ver [`super::palette`].
pub fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim();
    match value {
        "none" | "transparent" => return Some(Color::NONE),
        "white" => return Some(Color::WHITE),
        "black" => return Some(Color::BLACK),
        _ => {}
    }
    if let Some(hex) = value.strip_prefix('#') {
        let expand = |c: char| -> Option<u8> {
            let d = c.to_digit(16)? as u8;
            Some(d * 16 + d)
        };
        let bytes: Vec<u8> = match hex.len() {
            3 | 4 => hex.chars().map(expand).collect::<Option<Vec<_>>>()?,
            6 | 8 if hex.is_ascii() => (0..hex.len() / 2)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
                .collect::<Option<Vec<_>>>()?,
            _ => return None,
        };
        let f = |b: u8| b as f32 / 255.0;
        return Some(match bytes.len() {
            3 => Color::srgb(f(bytes[0]), f(bytes[1]), f(bytes[2])),
            4 => Color::srgba(f(bytes[0]), f(bytes[1]), f(bytes[2]), f(bytes[3])),
            _ => return None,
        });
    }
    if let Some(args) = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts: Vec<f32> = args
            .split(',')
            .map(|p| p.trim().parse::<f32>().ok())
            .collect::<Option<Vec<_>>>()?;
        if parts.len() < 3 {
            return None;
        }
        let alpha = parts.get(3).copied().unwrap_or(1.0);
        return Some(Color::srgba(
            parts[0] / 255.0,
            parts[1] / 255.0,
            parts[2] / 255.0,
            if alpha > 1.0 { alpha / 255.0 } else { alpha },
        ));
    }
    // Última tentativa: a paleta Tailwind (`slate-900`, `rose-400/80`).
    palette::resolve(value)
}

/// Multiplies a colour's alpha (used by the `opacity` property).
pub fn fade(color: Color, opacity: f32) -> Color {
    let mut c = color.to_srgba();
    c.alpha *= opacity;
    Color::Srgba(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Janela de referência para os testes de cascata.
    const VIEW: (f32, f32) = (1280.0, 720.0);

    #[test]
    fn test_parse_colors_in_every_accepted_form() {
        assert_eq!(parse_color("#fff"), Some(Color::srgb(1.0, 1.0, 1.0)));
        assert_eq!(parse_color("#000000"), Some(Color::srgb(0.0, 0.0, 0.0)));
        let half = parse_color("#ff000080").expect("8-digit hex");
        assert!((half.alpha() - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(parse_color("transparent"), Some(Color::NONE));
        assert!(parse_color("rgba(255, 0, 0, 0.5)").is_some());
        // Nonsense is rejected rather than silently black.
        assert_eq!(parse_color("chartreuse-ish"), None);
        assert_eq!(parse_color("#12345"), None);
        // Non-ASCII never reaches the byte slicing: a typo with an accented
        // char must be `None`, not a panic on a cut char boundary.
        assert_eq!(parse_color("#aáaa"), None);
        assert_eq!(parse_color("#áaaaaa"), None);
        assert_eq!(parse_color("#aaaaaá"), None);
    }

    #[test]
    fn test_parse_val_defaults_to_pixels() {
        assert_eq!(parse_val("12"), Some(Val::Px(12.0)));
        assert_eq!(parse_val("12px"), Some(Val::Px(12.0)));
        assert_eq!(parse_val("50%"), Some(Val::Percent(50.0)));
        assert_eq!(parse_val("auto"), Some(Val::Auto));
        assert_eq!(parse_val("wide"), None);
    }

    #[test]
    fn test_relative_font_units_wait_for_the_inheritance_pass() {
        // `%`, `em` e `rem` dependem da fonte do pai/raiz e só fecham na
        // aplicação (runtime) — o parser apenas as guarda. "rem" tem de
        // vencer "em" no desempate de sufixos.
        let pct = parse_declarations("font-size: 150%", "t");
        assert_eq!(pct.font_size, Some(Measure::plain(Val::Percent(150.0))));
        let em = parse_declarations("font-size: 1.2em", "t");
        assert_eq!(em.font_size_em, Some(1.2));
        assert!(em.font_size.is_none());
        let rem = parse_declarations("font-size: 0.9rem", "t");
        assert_eq!(rem.font_size_rem, Some(0.9));
        assert!(rem.font_size.is_none() && rem.font_size_em.is_none());
        // Absolutas continuam a chegar como Val.
        let abs = parse_declarations("font-size: 2vmin", "t");
        assert_eq!(abs.font_size, Some(Measure::plain(Val::VMin(2.0))));
    }

    #[test]
    fn test_inheritance_fills_only_what_the_cascade_left_open() {
        use bevy::color::palettes::basic::{GREEN, RED};
        let parent = StyleProps {
            color: Some(RED.into()),
            font_size: Some(Measure::plain(Val::Px(20.0))),
            text_align: Some(bevy::text::Justify::Center),
            ..Default::default()
        };
        // O filho que declara a própria cor mantém-na; o resto herda.
        let mut child = StyleProps {
            color: Some(GREEN.into()),
            ..Default::default()
        };
        child.apply_inherited(&parent);
        assert_eq!(child.color, Some(GREEN.into()));
        assert_eq!(child.font_size, Some(Measure::plain(Val::Px(20.0))));
        assert_eq!(child.text_align, Some(bevy::text::Justify::Center));
        assert_eq!(
            child.font_weight, None,
            "nem o pai nem o filho fixaram peso"
        );
    }

    #[test]
    fn test_parse_measure_accepts_functions_and_plain_vals() {
        assert_eq!(
            parse_measure("calc(10px + 4px)"),
            Some(Measure::Expr(Expr::Add(
                Box::new(Expr::Px(10.0)),
                Box::new(Expr::Px(4.0))
            )))
        );
        assert_eq!(parse_measure("24"), Some(Measure::plain(Val::Px(24.0))));
        assert_eq!(parse_measure("40vw"), Some(Measure::plain(Val::Vw(40.0))));
        // `%` precisa do tamanho do PAI — recusado com warn, declaração nula.
        assert_eq!(parse_measure("calc(10% + 2px)"), None);
    }

    #[test]
    fn test_calc_min_max_clamp_resolve_against_the_authored_viewport() {
        let mut props = parse_declarations(
            "width: calc(50vw - 13px); gap: min(8px, 1vmin); \
             font-size: clamp(14px, 2vmin, 22px)",
            "t",
        );
        // Janela 1280×720: vw=12.8, vmin=7.2.
        props.resolve_viewport((1280.0, 720.0));
        assert_eq!(props.width, Some(Measure::plain(Val::Px(627.0))));
        assert_eq!(props.row_gap, Some(Measure::plain(Val::Px(7.2))));
        assert_eq!(props.column_gap, props.row_gap);
        assert_eq!(props.font_size, Some(Measure::plain(Val::Px(14.4))));
    }

    #[test]
    fn test_expressions_keep_precedence_parentheses_and_negatives() {
        let mut props = parse_declarations(
            "width: calc(2 * (100px + 5px) - 4px); \
             top: min(max(30px, 10px), 15px); \
             bottom: calc(-4px + 10px)",
            "t",
        );
        props.resolve_viewport((1280.0, 720.0));
        assert_eq!(props.width, Some(Measure::plain(Val::Px(206.0))));
        assert_eq!(props.top, Some(Measure::plain(Val::Px(15.0))));
        assert_eq!(props.bottom, Some(Measure::plain(Val::Px(6.0))));
    }

    #[test]
    fn test_division_by_zero_drops_the_declaration() {
        let mut props = parse_declarations("width: calc(10px / 0)", "t");
        props.resolve_viewport((1280.0, 720.0));
        assert_eq!(props.width, None);
    }

    #[test]
    fn test_parse_val_viewport_units() {
        assert_eq!(parse_val("40vw"), Some(Val::Vw(40.0)));
        assert_eq!(parse_val("100vh"), Some(Val::Vh(100.0)));
        assert_eq!(parse_val("6vmin"), Some(Val::VMin(6.0)));
        assert_eq!(parse_val("30 vmax"), Some(Val::VMax(30.0)));
        // Decimais também — 4.5vh é um offset perfeitamente normal.
        assert_eq!(parse_val("4.5vh"), Some(Val::Vh(4.5)));
        // `v` solto não é unidade.
        assert_eq!(parse_val("10v"), None);
    }

    #[test]
    fn test_resolve_viewport_converts_units_to_pixels() {
        // A regressão que meteu o HUD "todo torto": vmin resolvido com o MAX
        // da janela pelo caminho de layout da Bevy, e font-size com unidades
        // simplesmente ignorado. Ambos resolvem AGORA contra a janela dada.
        let mut props = parse_declarations(
            "width: 44vmin; height: 10vh; left: 3vw; padding: 2vmin 4vmax; \
             font-size: 2.2vmin; box-shadow: 0 1vmin 3vmin #00000088",
            "t",
        );
        // Janela 1280×720: vmin=7.2, vmax=12.8. (f32: comparação com folga)
        props.resolve_viewport((1280.0, 720.0));
        let px = |v: f32| Val::Px(v);
        let close = |a: Option<Measure>, b: f32| match a {
            Some(Measure::Plain(Val::Px(v))) => (v - b).abs() < 0.01,
            _ => false,
        };
        assert!(close(props.width, 316.8));
        assert!(close(props.height, 72.0));
        assert!(close(props.left, 38.4));
        let padding = props.padding.expect("padding");
        assert!(matches!(padding.top, Val::Px(v) if (v - 14.4).abs() < 0.01));
        assert!(matches!(padding.left, Val::Px(v) if (v - 51.2).abs() < 0.01));
        assert!(close(props.font_size, 15.84));
        let shadow = props.box_shadow.expect("shadow")[0];
        assert!(matches!(shadow.y_offset, Val::Px(v) if (v - 7.2).abs() < 0.01));
        assert!(matches!(shadow.blur_radius, Val::Px(v) if (v - 21.6).abs() < 0.01));
        // Janela ESTREITA (600×1200): vmin=6 — o mesmo CSS adapta-se.
        let mut tall = parse_declarations("width: 44vmin", "t");
        tall.resolve_viewport((600.0, 1200.0));
        assert!(close(tall.width, 264.0));
        // Px e % passam intactos.
        let mut flat = parse_declarations("width: 200; height: 50%", "t");
        flat.resolve_viewport((1280.0, 720.0));
        assert_eq!(flat.width, Some(Measure::plain(px(200.0))));
        assert_eq!(flat.height, Some(Measure::plain(Val::Percent(50.0))));
    }

    #[test]
    fn test_media_conditions_match_against_the_window() {
        let cond = parse_media_cond("(min-width: 900) and (max-width: 1600)").expect("cond");
        assert!(cond.matches(1280.0, 720.0));
        assert!(!cond.matches(800.0, 720.0), "abaixo do mínimo");
        assert!(!cond.matches(1920.0, 1080.0), "acima do máximo");
        // Aspecto e orientação.
        let portrait = parse_media_cond("(portrait)").expect("portrait");
        assert!(portrait.matches(720.0, 1280.0));
        assert!(!portrait.matches(1280.0, 720.0));
        let wide = parse_media_cond("(landscape) and (min-aspect: 1.7)").expect("wide");
        assert!(wide.matches(1920.0, 1080.0));
        assert!(wide.matches(1280.0, 720.0), "1.78 ≥ 1.7 passa");
        assert!(!wide.matches(1024.0, 768.0), "1.33 < 1.7 falha");
        // Rejeição honesta do desconhecido.
        assert!(parse_media_cond("(hover)").is_none());
    }

    #[test]
    fn test_media_blocks_scope_rules_and_swap_on_resize() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(
            ".card { background: #111111 }
             @media (min-width: 900) { .card { background: #222222; width: 40vw } }
             @media (max-width: 899) { .card { width: 90vw } }",
        );
        assert_eq!(sheet.rules.len(), 3);
        let classes: Vec<String> = vec!["card".into()];
        // Janela larga: a regra do bloco min-width entra (ordem desempata).
        let wide = sheet.resolve_one(
            "uipanel",
            None,
            &classes,
            StyleState::Normal,
            (1280.0, 720.0),
        );
        assert_eq!(wide.background, parse_color("#222222"));
        assert_eq!(wide.width, Some(Measure::plain(Val::Vw(40.0))));
        // Janela estreita: o bloco min-width desaparece POR INTEIRO da cascata.
        let narrow = sheet.resolve_one(
            "uipanel",
            None,
            &classes,
            StyleState::Normal,
            (640.0, 960.0),
        );
        assert_eq!(narrow.background, parse_color("#111111"));
        assert_eq!(narrow.width, Some(Measure::plain(Val::Vw(90.0))));
    }

    #[test]
    fn test_nested_media_blocks_close_at_the_right_brace() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(
            "@media (min-width: 500) {
                 .a { color: #ff0000 }
                 @media (portrait) { .b { color: #00ff00 } }
                 .c { color: #0000ff }
             }
             .d { color: #ffff00 }",
        );
        // .a e .c com a condição externa; .b com as duas empilhadas; .d solto.
        assert_eq!(sheet.rules.len(), 4);
        let media_of_class = |class: &str| {
            sheet
                .rules
                .iter()
                .find(|r| r.selector.parts[0].classes.iter().any(|c| c == class))
                .and_then(|r| r.media.clone())
        };
        let a = media_of_class("a");
        let b = media_of_class("b");
        let d = media_of_class("d");
        let a = a.expect("a vive no bloco externo");
        assert_eq!(a.min_width, Some(500.0));
        let b = b.expect("b tem media empilhada");
        assert_eq!(b.min_width, Some(500.0));
        assert!(b.portrait, "a condição interna empilha na externa");
        assert!(
            d.is_none(),
            "regra de fora de qualquer bloco é incondicional"
        );
    }

    #[test]
    fn test_parse_rect_follows_the_css_shorthand() {
        assert_eq!(parse_rect("4"), Some(UiRect::all(Val::Px(4.0))));
        // 2 values = vertical horizontal
        let two = parse_rect("10 8").expect("two values");
        assert_eq!(two.top, Val::Px(10.0));
        assert_eq!(two.left, Val::Px(8.0));
        let four = parse_rect("1 2 3 4").expect("four values");
        assert_eq!(
            (four.top, four.right, four.bottom, four.left),
            (Val::Px(1.0), Val::Px(2.0), Val::Px(3.0), Val::Px(4.0))
        );
    }

    #[test]
    fn test_selector_parsing_and_specificity() {
        let id = parse_selector("#hp").expect("id selector");
        let class = parse_selector(".chip").expect("class selector");
        let tag = parse_selector("UiText").expect("tag selector");
        assert!(id.specificity() > class.specificity());
        assert!(class.specificity() > tag.specificity());
        // Tag selectors are case-insensitive, matching the XML tag handling.
        assert_eq!(tag.parts[0].tag.as_deref(), Some("uitext"));
        // A pseudo-class adds a class's worth of weight.
        let hover = parse_selector(".chip:hover").expect("pseudo");
        assert_eq!(hover.parts[0].state, StyleState::Hover);
        assert!(hover.specificity() > class.specificity());
    }

    #[test]
    fn test_cascade_orders_tag_then_class_then_id() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(
            "uipanel { background: #111111; width: 10 }
             .card   { background: #222222 }
             #hero   { background: #333333 }",
        );
        let props = sheet.resolve_one(
            "uipanel",
            Some("hero"),
            &["card".into()],
            StyleState::Normal,
            VIEW,
        );
        assert_eq!(props.background, parse_color("#333333"));
        // The tag rule still contributes what nothing else overrode.
        assert_eq!(props.width, Some(Measure::plain(Val::Px(10.0))));
    }

    #[test]
    fn test_source_order_breaks_specificity_ties() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(".a { color: #ff0000 } .b { color: #00ff00 }");
        let props = sheet.resolve_one(
            "uitext",
            None,
            &["a".into(), "b".into()],
            StyleState::Normal,
            VIEW,
        );
        assert_eq!(props.color, parse_color("#00ff00"));
    }

    #[test]
    fn test_state_rules_only_apply_in_their_state() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(".btn { background: #101010 } .btn:hover { background: #f0f0f0 }");
        let normal = sheet.resolve_one("uipanel", None, &["btn".into()], StyleState::Normal, VIEW);
        assert_eq!(normal.background, parse_color("#101010"));
        let hover = sheet.resolve_one("uipanel", None, &["btn".into()], StyleState::Hover, VIEW);
        assert_eq!(hover.background, parse_color("#f0f0f0"));
    }

    /// Convenience for the descendant tests: an element with tag + classes.
    fn element<'a>(tag: &'a str, classes: &'a [String]) -> ElementRef<'a> {
        ElementRef {
            tag,
            id: None,
            classes,
            state: StyleState::Normal,
        }
    }

    #[test]
    fn test_the_examples_stylesheet_parses_and_resolves() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(include_str!("../../examples/simple-rpg/ui/theme.css"));
        sheet.parse_into(include_str!("../../examples/simple-rpg/ui/hud.css"));
        sheet.parse_into(include_str!("../../examples/simple-rpg/ui/theme.css"));
        sheet.parse_into(include_str!("../../examples/simple-rpg/ui/menu.css"));
        let root = ElementRef {
            tag: "uiroot",
            id: Some("hud"),
            classes: &[],
            state: StyleState::Normal,
        };
        let resolve = |class: &str, viewport| {
            let classes = vec![class.to_string()];
            sheet.resolve(
                &[root.clone(), element("uipanel", &classes)],
                viewport,
                None,
            )
        };
        let card = resolve("hud-card", VIEW);
        assert!(card.background.is_some());
        assert!(card.box_shadow.is_some_and(|s| !s.is_empty()));
        let vitals = resolve("anchor-vitals", VIEW);
        assert_eq!(vitals.top, Some(Measure::plain(Val::Px(18.0))));
        assert_eq!(vitals.left, Some(Measure::plain(Val::Px(18.0))));
        assert_eq!(vitals.bottom, None);
        let dock = resolve("anchor-dock", VIEW);
        assert_eq!(dock.bottom, Some(Measure::plain(Val::Px(18.0))));
        assert_eq!(dock.right, Some(Measure::plain(Val::Px(18.0))));
        assert_eq!(dock.width, Some(Measure::plain(Val::Px(426.0))));
        let narrow = resolve("anchor-dock", (600.0, 1000.0));
        assert_eq!(narrow.max_width, Some(Measure::plain(Val::Vw(94.0))));
        assert_eq!(narrow.bottom, Some(Measure::plain(Val::Px(12.0))));
        assert_eq!(
            resolve("anchor-clock", VIEW).bottom,
            Some(Measure::plain(Val::Px(174.0)))
        );
        assert_eq!(
            resolve("anchor-clock", (600.0, 1000.0)).bottom,
            Some(Measure::plain(Val::Px(298.0)))
        );
        let menu = sheet.resolve_one(
            "uipanel",
            None,
            &["hud-card".into()],
            StyleState::Normal,
            VIEW,
        );
        assert!(
            menu.background.is_none(),
            "HUD styles must not leak into the journal"
        );
        // Retrato 1000×1600 com a escala a limitar pela largura: o espaço
        // autoral é 720×1152 e é nele que a faixa estreita avalia (ver
        // scale::ui_viewport — os testes de runtime cobrem a conversão).
        let portrait = resolve("anchor-target", (720.0, 1152.0));
        assert_eq!(portrait.top, Some(Measure::plain(Val::Px(198.0))));
        let stock_classes = vec![
            "action-slot".into(),
            "consumable".into(),
            "depleted".into(),
        ];
        let stock = sheet.resolve(
            &[root, element("uipanel", &stock_classes)],
            (1920.0, 1080.0),
            None,
        );
        assert_eq!(stock.width, Some(Measure::plain(Val::Px(48.0))));
        assert_eq!(stock.height, Some(Measure::plain(Val::Px(46.0))));
        assert!(
            stock.padding.is_none(),
            "empty menu placeholders must not resize consumables"
        );
    }

    #[test]
    fn test_compound_selector_requires_every_class() {
        let selector = parse_selector(".track.hp").expect("compound");
        let both: Vec<String> = vec!["track".into(), "hp".into()];
        let one: Vec<String> = vec!["track".into()];
        assert!(selector.matches(&[element("uipanel", &both)]));
        assert!(!selector.matches(&[element("uipanel", &one)]));
        // Two classes outweigh a single-class rule.
        assert!(selector.specificity() > parse_selector(".track").unwrap().specificity());
    }

    #[test]
    fn test_descendant_selector_matches_at_any_depth() {
        let selector = parse_selector(".panel .fill").expect("descendant");
        let panel: Vec<String> = vec!["panel".into()];
        let middle: Vec<String> = vec!["inner".into()];
        let fill: Vec<String> = vec!["fill".into()];
        // Direct child…
        assert!(selector.matches(&[element("uipanel", &panel), element("uipanel", &fill)]));
        // …and a grandchild.
        assert!(selector.matches(&[
            element("uipanel", &panel),
            element("uipanel", &middle),
            element("uipanel", &fill),
        ]));
        // The ancestor really has to be there.
        assert!(!selector.matches(&[element("uipanel", &middle), element("uipanel", &fill)]));
        // And the order matters: a `.panel` inside a `.fill` is not a match.
        assert!(!selector.matches(&[element("uipanel", &fill), element("uipanel", &panel)]));
    }

    #[test]
    fn test_descendant_rules_resolve_through_the_chain() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(
            ".track .fill { background: #ff0000 }
             .track.danger .fill { background: #00ff00 }",
        );
        let calm: Vec<String> = vec!["track".into()];
        let hurt: Vec<String> = vec!["track".into(), "danger".into()];
        let fill: Vec<String> = vec!["fill".into()];
        let normal = sheet.resolve(
            &[element("uipanel", &calm), element("uipanel", &fill)],
            VIEW,
            None,
        );
        assert_eq!(normal.background, parse_color("#ff0000"));
        // Toggling a class on the ANCESTOR restyles the descendant — the thing
        // the HUD relies on for "health is low".
        let danger = sheet.resolve(
            &[element("uipanel", &hurt), element("uipanel", &fill)],
            VIEW,
            None,
        );
        assert_eq!(danger.background, parse_color("#00ff00"));
    }

    #[test]
    fn test_tag_and_id_combine_in_one_compound() {
        let selector = parse_selector("UiPanel#hp.card").expect("compound");
        let part = &selector.parts[0];
        assert_eq!(part.tag.as_deref(), Some("uipanel"));
        assert_eq!(part.id.as_deref(), Some("hp"));
        assert_eq!(part.classes, vec!["card".to_string()]);
        assert_eq!(part.specificity(), 100 + 10 + 1);
    }

    #[test]
    fn test_text_shadow_parses_offsets_colour_and_none() {
        let one = parse_declarations("text-shadow: 2 #112233ff", "t").text_shadow;
        let spec = one.expect("declared").expect("not none");
        assert_eq!(spec.offset, Vec2::splat(2.0));
        // Dois números são dx dy — a sombra do HUD é deslocada, não centrada.
        let two = parse_declarations("text-shadow: 1 3", "t").text_shadow;
        assert_eq!(two.unwrap().unwrap().offset, Vec2::new(1.0, 3.0));
        // `none` é explícito: remove uma sombra herdada da cascata.
        let none = parse_declarations("text-shadow: none", "t").text_shadow;
        assert!(none.expect("declared").is_none());
        // Ausente ≠ none: a regra simplesmente não fala do assunto.
        assert!(parse_declarations("color: #fff", "t").text_shadow.is_none());
    }

    #[test]
    fn test_hover_on_an_ancestor_scopes_the_descendant() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(".slot:hover .veil { opacity: 0.2 }");
        let slot: Vec<String> = vec!["slot".into()];
        let veil: Vec<String> = vec!["veil".into()];
        let idle = sheet.resolve(
            &[element("uipanel", &slot), element("uipanel", &veil)],
            VIEW,
            None,
        );
        assert_eq!(idle.opacity, None);
        let hovered = sheet.resolve(
            &[
                ElementRef {
                    state: StyleState::Hover,
                    ..element("uipanel", &slot)
                },
                element("uipanel", &veil),
            ],
            VIEW,
            None,
        );
        assert_eq!(hovered.opacity, Some(0.2));
    }

    #[test]
    fn test_a_rule_only_applies_to_roots_that_load_its_sheet() {
        // O incidente `.empty`: uma classe comum a duas folhas vazava de uma
        // raiz para a outra. Com scoping, a folha só vê as raízes que a
        // carregam — e `None` (testes/`resolve_one`) continua a ver tudo.
        let mut sheet = StyleSheet::default();
        let theme = sheet.parse_into(".chip { background: #111111 }");
        let hud = sheet.parse_into(".chip { background: #222222 }");
        assert_eq!(theme, 0);
        assert_eq!(hud, 1);
        let chip = vec!["chip".into()];
        let chain = [element("uipanel", &chip)];
        assert_eq!(
            sheet.resolve(&chain, VIEW, Some(&[0])).background,
            parse_color("#111111"),
            "menu carrega só o tema"
        );
        assert_eq!(
            sheet.resolve(&chain, VIEW, Some(&[0, 1])).background,
            parse_color("#222222"),
            "hud carrega as duas; a sua própria vence"
        );
        assert_eq!(
            sheet.resolve(&chain, VIEW, Some(&[])).background,
            None,
            "raiz sem folhas: nada se aplica"
        );
        assert_eq!(
            sheet.resolve(&chain, VIEW, None).background,
            parse_color("#222222")
        );
    }

    #[test]
    fn test_malformed_compounds_are_rejected_not_half_parsed() {
        assert!(parse_selector(".").is_none());
        assert!(parse_selector("#").is_none());
        assert!(parse_selector("#a#b").is_none(), "two ids is nonsense");
        assert!(parse_selector(".a > .b").is_none(), "no child combinator");
    }

    #[test]
    fn test_comments_and_malformed_rules_do_not_kill_the_sheet() {
        let mut sheet = StyleSheet::default();
        sheet.parse_into(
            "/* header */
             .ok { color: #ffffff; bogus-prop: 3; not a declaration }
             .also-ok { font-size: 18 }",
        );
        assert_eq!(sheet.rules.len(), 2);
        let ok = sheet.resolve_one("uitext", None, &["ok".into()], StyleState::Normal, VIEW);
        assert_eq!(ok.color, parse_color("#ffffff"));
        let also = sheet.resolve_one(
            "uitext",
            None,
            &["also-ok".into()],
            StyleState::Normal,
            VIEW,
        );
        assert_eq!(also.font_size, Some(Measure::plain(Val::Px(18.0))));
    }

    #[test]
    fn test_border_shorthand_sets_width_and_colour() {
        let props = parse_declarations("border: 1.5 #d8b46a", "test");
        assert_eq!(props.border, Some(UiRect::all(Val::Px(1.5))));
        assert_eq!(props.border_color, parse_color("#d8b46a"));
    }

    #[test]
    fn test_merge_only_overwrites_what_the_patch_sets() {
        let mut base = parse_declarations("width: 10; color: #ffffff", "base");
        let patch = parse_declarations("color: #ff0000", "patch");
        base.merge(&patch);
        assert_eq!(
            base.width,
            Some(Measure::plain(Val::Px(10.0))),
            "width survives the merge"
        );
        assert_eq!(base.color, parse_color("#ff0000"));
    }

    #[test]
    fn test_apply_to_node_writes_the_layout_half() {
        let props = parse_declarations(
            "direction: column; width: 100%; padding: 6 10; gap: 4; position: absolute; top: 12",
            "test",
        );
        let mut node = Node::default();
        props.apply_to_node(&mut node);
        assert_eq!(node.flex_direction, FlexDirection::Column);
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.padding.top, Val::Px(6.0));
        assert_eq!(node.padding.left, Val::Px(10.0));
        assert_eq!(node.row_gap, Val::Px(4.0));
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!(node.top, Val::Px(12.0));
    }

    #[test]
    fn test_palette_names_resolve_through_parse_color() {
        // O mesmo valor que a paleta Tailwind documenta para slate-900.
        let slate = parse_color("slate-900").expect("slate-900");
        assert!((slate.to_srgba().red - 0x0f as f32 / 255.0).abs() < 1e-5);
        let dim = parse_color("rose-400/25").expect("dimmed");
        assert!((dim.to_srgba().alpha - 0.25).abs() < 1e-6);
        // O resto do dialecto continua a funcionar.
        assert!(parse_color("#ff0000").is_some());
    }

    #[test]
    fn test_box_shadows_parse_offsets_blur_spread_and_color() {
        let one = parse_declarations("box-shadow: 0 4 12 #00000066", "t")
            .box_shadow
            .expect("declared");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].x_offset, Val::Px(0.0));
        assert_eq!(one[0].y_offset, Val::Px(4.0));
        assert_eq!(one[0].blur_radius, Val::Px(12.0));
        assert_eq!(one[0].spread_radius, Val::Px(0.0));
        assert_eq!(one[0].color, parse_color("#00000066").expect("colour"));

        // Duas sombras numa declaração, com spread a seguir ao blur.
        let two = parse_declarations("box-shadow: 2 4 8 1 #00000088, 0 0 2 #ffffff33", "t")
            .box_shadow
            .expect("declared");
        assert_eq!(two.len(), 2);
        assert_eq!(two[0].spread_radius, Val::Px(1.0));
        // `none` é uma lista vazia — sombra explicitamente desligada.
        assert_eq!(
            parse_declarations("box-shadow: none", "t").box_shadow,
            Some(Vec::new())
        );
    }

    #[test]
    fn test_grid_tracks_parse_px_percent_fr_and_repeat() {
        let tracks = parse_tracks("64 1fr 25% auto").expect("tracks");
        assert_eq!(tracks.len(), 4);
        // repeat(4, 1fr) tem de bater com o constructor directo — os campos
        // são privados, a comparação estrutural é o contrato.
        assert_eq!(
            parse_tracks("repeat(4, 1fr)"),
            Some(vec![RepeatedGridTrack::fr(4, 1.0)])
        );
        assert_eq!(
            parse_tracks("repeat(2, 40)"),
            Some(vec![RepeatedGridTrack::px(2, 40.0)])
        );
        // Uma pista malformada não leva a lista abaixo — mas uma lista só de
        // lixo é um None, para o autor ver o aviso.
        assert!(parse_tracks("banana").is_none());
    }

    #[test]
    fn test_grid_placements_parse_the_css_forms() {
        let auto = parse_placement("auto").expect("auto");
        assert_eq!(auto, GridPlacement::default());
        assert_eq!(parse_placement("2"), Some(GridPlacement::start(2)));
        assert_eq!(parse_placement("span 3"), Some(GridPlacement::span(3)));
        assert_eq!(
            parse_placement("2 / span 3"),
            Some(GridPlacement::start_span(2, 3))
        );
        assert_eq!(
            parse_placement("1 / 4"),
            Some(GridPlacement::start_end(1, 4))
        );
        assert_eq!(parse_placement("nope"), None);
    }

    #[test]
    fn test_grid_declarations_land_in_the_node() {
        let mut node = Node::default();
        parse_declarations(
            "display: grid; grid-template-columns: repeat(3, 1fr); grid-template-rows: 40 40; \
             grid-auto-flow: row dense; grid-column: 1 / span 2; grid-row: 2",
            "t",
        )
        .apply_to_node(&mut node);
        assert_eq!(node.display, Display::Grid);
        assert_eq!(node.grid_template_columns.len(), 1);
        assert_eq!(node.grid_template_rows.len(), 2);
        assert_eq!(node.grid_auto_flow, GridAutoFlow::RowDense);
        assert_eq!(node.grid_column, GridPlacement::start_span(1, 2));
        assert_eq!(node.grid_row, GridPlacement::start(2));
    }

    #[test]
    fn test_font_weight_underline_and_linebreak_parse() {
        let props = parse_declarations(
            "font-weight: bold; underline: 1; line-break: nowrap; translate: 0 -4",
            "t",
        );
        assert_eq!(props.font_weight, Some(700.0));
        assert_eq!(
            parse_declarations("font-weight: 600", "t").font_weight,
            Some(600.0),
            "números também são aceites"
        );
        assert_eq!(
            props.text_underline,
            Some(Decoration::On(None)),
            "`underline: 1` liga com a cor do texto"
        );
        assert_eq!(props.linebreak, Some(LineBreak::NoWrap));
        assert_eq!(props.translate, Some(Vec2::new(0.0, -4.0)));
        // `none` desliga explicitamente um sublinhado herdado.
        assert_eq!(
            parse_declarations("underline: none", "t").text_underline,
            Some(Decoration::Off)
        );
    }

    #[test]
    fn test_cursor_and_pointer_events_parse() {
        let props = parse_declarations("cursor: pointer; pointer-events: none", "t");
        assert!(props.cursor.is_some(), "pointer é um cursor conhecido");
        assert_eq!(props.pointer_none, Some(true));
        assert_eq!(
            parse_declarations("cursor: banana", "t").cursor,
            None,
            "cursor desconhecido é recusado, não silenciosamente default"
        );
    }
}
