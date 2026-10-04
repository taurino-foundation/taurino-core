use std::collections::HashSet;
use wry::WebContext as WryWebContext;

#[derive(Debug)]
pub struct WebContext {
  pub inner: WryWebContext,
  pub referenced_by_webviews: HashSet<String>,
  // on Linux the custom protocols are associated with the context
  // and you cannot register a URI scheme more than once
  pub registered_custom_protocols: HashSet<String>,
}
