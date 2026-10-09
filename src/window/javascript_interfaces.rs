use anyhow::Result;
use serde::Serialize;
use serialize_to_javascript::{Options, Serialized};
use wry::WebView;
pub struct JsEvaluator<'a> {
  webview: &'a WebView,
}

impl<'a> JsEvaluator<'a> {
  pub fn new(webview: &'a WebView) -> Self {
    Self { webview }
  }

  pub fn evaluate(&self, script: &str) -> Result<()> {
    self.webview.evaluate_script(script)?;
    Ok(())
  }

  fn callback<T: Serialize>(&self, id: u32, data: &T) -> Result<()> {
    let raw = serde_json::value::to_raw_value(data)?;

    let js = Serialized::new(&raw, &Options::default());

    self.evaluate(&format!(
      "window.__TAURINO_INTERNALS__.runCallback({id}, {});",
      js.into_string(),
    ))
  }

  pub fn resolve<T: Serialize>(&self, id: u32, value: &T) -> Result<()> {
    self.callback(id, value)
  }

  pub fn reject<T: Serialize>(&self, id: u32, error: &T) -> Result<()> {
    self.callback(id, error)
  }
}
