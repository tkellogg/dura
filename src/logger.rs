use chrono::Utc;
use serde::ser::SerializeMap;
use serde::Serializer;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::Layer;

pub struct NestedJsonLayer<W: for<'a> MakeWriter<'a> + 'static> {
    mw: W,
}

impl<W: for<'a> MakeWriter<'a> + 'static> NestedJsonLayer<W> {
    pub fn new(mw: W) -> Self {
        Self { mw }
    }

    pub fn serialize_and_write(
        &self,
        event: &tracing::Event<'_>,
        hm: BTreeMap<&'static str, serde_json::Value>,
    ) -> Result<Vec<u8>, serde_json::Error> {
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buffer);
        let mut ser_map = serializer.serialize_map(None)?;

        ser_map.serialize_entry("target", event.metadata().target())?;
        ser_map.serialize_entry("file", &event.metadata().file())?;
        ser_map.serialize_entry("name", event.metadata().name())?;
        ser_map.serialize_entry("level", &format!("{:?}", event.metadata().level()))?;
        ser_map.serialize_entry("fields", &hm)?;
        ser_map.serialize_entry("time", &Utc::now().to_rfc3339())?;
        ser_map.end()?;
        Ok(buffer)
    }

    pub fn write_all(&self, mut buffer: Vec<u8>) -> std::io::Result<()> {
        buffer.write_all(b"\n")?;
        self.mw.make_writer().write_all(&buffer)
    }
}

impl<W, S> Layer<S> for NestedJsonLayer<W>
where
    S: Subscriber,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        if let Ok(buffer) = self.serialize_and_write(event, visitor.0) {
            {
                let _ = self.write_all(buffer);
            }
        }
    }
}

struct JsonVisitor(BTreeMap<&'static str, serde_json::Value>);

impl Default for JsonVisitor {
    fn default() -> Self {
        JsonVisitor(Default::default())
    }
}

impl Visit for JsonVisitor {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.insert(field.name(), value.into());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name(), value.into());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.insert(field.name(), value.into());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match serde_json::from_str::<serde_json::Value>(value) {
            Ok(value) => {
                self.0.insert(field.name(), value.into());
            }
            Err(_) => {
                self.0.insert(field.name(), value.to_string().into());
            }
        }
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.0.insert(field.name(), value.to_string().into());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let s = format!("{:?}", value);
        match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(value) => {
                self.0.insert(field.name(), value.into());
            }
            Err(_) => {
                self.0.insert(field.name(), s.into());
            }
        }
    }
}
