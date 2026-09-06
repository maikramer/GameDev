//! Captura de logs (`tracing`) num ring-buffer — o "console" do bridge.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Subscriber};
use bevy::log::tracing_subscriber::Layer as TracingLayer;
use bevy::log::tracing_subscriber::layer::Context;
use bevy::log::{BoxedLayer, LogPlugin};
use serde::Serialize;

/// Entradas guardadas por `viber.logs` (ordem cronológica, cortadas por limite).
#[derive(Clone, Serialize)]
pub struct LogEntry {
    /// Segundos UNIX (float) da captura.
    pub time: f64,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Ring-buffer compartilhado alimentado pela layer de tracing.
pub type LogBuffer = Arc<Mutex<VecDeque<LogEntry>>>;

/// Máximo de entradas retidas (`viber.logs` devolve as últimas).
pub const LOG_BUFFER_CAPACITY: usize = 1000;

/// Cap por entrada: um log gigante (ex.: `viber.log(string.rep("x", 10^7))`
/// de um script) não pode reter dezenas de MB por linha num ring de 1000 —
/// trunca em char boundary antes de construir a [`LogEntry`].
const MAX_LOG_MESSAGE: usize = 8192;

/// Buffer global do processo — o `LogPlugin::custom_layer` é um fn pointer
/// (sem captures), logo a layer lê daqui; `BridgeShared` referencia o mesmo.
pub fn global_log_buffer() -> LogBuffer {
    static BUFFER: OnceLock<LogBuffer> = OnceLock::new();
    BUFFER
        .get_or_init(|| Arc::new(Mutex::new(VecDeque::with_capacity(LOG_BUFFER_CAPACITY))))
        .clone()
}

/// Layer de `tracing` que copia eventos para o buffer do bridge.
pub struct BridgeLogLayer {
    pub buffer: LogBuffer,
}

struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }
}

impl<S: Subscriber> TracingLayer<S> for BridgeLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            return;
        }
        if visitor.message.len() > MAX_LOG_MESSAGE {
            let mut end = MAX_LOG_MESSAGE;
            while !visitor.message.is_char_boundary(end) {
                end -= 1;
            }
            visitor.message.truncate(end);
            visitor.message.push('…');
        }
        let entry = LogEntry {
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0),
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_string(),
            message: visitor.message,
        };
        if let Ok(mut buffer) = self.buffer.lock() {
            if buffer.len() >= LOG_BUFFER_CAPACITY {
                buffer.pop_front();
            }
            buffer.push_back(entry);
        }
    }
}

/// `LogPlugin` com a layer do bridge instalada (campo `custom_layer`).
///
/// Uso: `DefaultPlugins.set(log_plugin_with_bridge())`.
pub fn log_plugin_with_bridge() -> LogPlugin {
    LogPlugin {
        custom_layer: |_app| {
            Some(Box::new(BridgeLogLayer {
                buffer: global_log_buffer(),
            }) as BoxedLayer)
        },
        ..Default::default()
    }
}
