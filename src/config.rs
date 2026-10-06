use anyhow::Result;
use url::Url;

use crate::{
  schema::{FrontendDist, window::WindowConfig},
  utils::Env,
};

#[derive(Debug, Clone)]
pub struct Config {
  env: Env,
  frontend_dist: FrontendDist,
  windows: Vec<WindowConfig>,
}

impl Config {
  pub fn new(_config: &str) -> Result<Self> {
    let env = Env::default();

    let frontend_dist = FrontendDist::Url(Url::parse("https://tauri.app")?); // Directory("dist".into());

    Ok(Self {
      env,
      frontend_dist,
      windows: Vec::new(),
    })
  }
  pub fn add_window_config(&mut self, config: WindowConfig) {
    self.windows.push(config);
  }

  pub fn get_env(&self) -> Env {
    self.env.clone()
  }
  pub fn get_windows(&self) -> &[WindowConfig] {
    &self.windows
  }
  pub fn frontend_dist(&self) -> FrontendDist {
    self.frontend_dist.clone()
  }
}
