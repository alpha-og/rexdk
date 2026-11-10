use crate::runtime::api::{control::ControlMessage, data::DataMessage};

/// A message from either Rex protocol family.
///
/// Control messages are JSON text on `/api/control`; data messages are binary
/// frames on `/api/data`. There is no shared wire encoding, so this enum only
/// groups the two families and has no serde representation of its own.
///
/// Messages are parsed by family through [`ControlMessage`] and
/// [`DataMessage`]; the wrapper exists to name the whole protocol surface.
#[derive(Debug)]
#[allow(dead_code, clippy::large_enum_variant)]
pub enum Message {
    /// A control-connection message.
    Control(ControlMessage),
    /// A data-connection message.
    Data(DataMessage),
}

impl From<ControlMessage> for Message {
    fn from(message: ControlMessage) -> Self {
        Self::Control(message)
    }
}

impl From<DataMessage> for Message {
    fn from(message: DataMessage) -> Self {
        Self::Data(message)
    }
}
