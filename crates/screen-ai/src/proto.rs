//! The `chrome_screen_ai.VisualAnnotation` message, reduced to the fields read here.

/// The result of one `PerformOCR` call.
#[derive(Clone, PartialEq, prost::Message)]
pub struct VisualAnnotation {
    /// The recognised lines.
    #[prost(message, repeated, tag = "2")]
    pub lines: Vec<LineBox>,
}

/// One recognised line.
#[derive(Clone, PartialEq, prost::Message)]
pub struct LineBox {
    /// The box of the line, in pixels of the recognised image.
    #[prost(message, optional, tag = "2")]
    pub bounding_box: Option<Rect>,
    /// The text of the line.
    #[prost(string, tag = "3")]
    pub utf8_string: String,
    /// The block the line belongs to.
    #[prost(int32, tag = "5")]
    pub block_id: i32,
    /// The paragraph of the block the line belongs to.
    #[prost(int32, tag = "11")]
    pub paragraph_id: i32,
}

/// A box in pixels.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Rect {
    /// Left edge.
    #[prost(int32, tag = "1")]
    pub x: i32,
    /// Top edge.
    #[prost(int32, tag = "2")]
    pub y: i32,
    /// Width.
    #[prost(int32, tag = "3")]
    pub width: i32,
    /// Height.
    #[prost(int32, tag = "4")]
    pub height: i32,
}
