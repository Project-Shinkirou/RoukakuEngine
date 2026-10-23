use roukaku_ui_core::UiValidationError;

#[derive(Debug, Clone, PartialEq)]
pub struct RoukakuTextMeasureRequest {
    pub semantic_id: String,
    pub text: String,
    pub max_width: f32,
    pub font_size: f32,
    pub max_lines: u32,
    pub direction: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoukakuTextMeasureResult {
    pub width: f32,
    pub height: f32,
}

pub trait RoukakuTextMeasurer: Send + Sync {
    fn measure(
        &self,
        request: &RoukakuTextMeasureRequest,
    ) -> Result<RoukakuTextMeasureResult, UiValidationError>;
}
