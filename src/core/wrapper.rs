/// Wrapper around a [`tao::window::Icon`] that can be created from an [`Icon`].
pub struct TaoIcon(pub TaoWindowIcon);

impl TryFrom<Icon<'_>> for TaoIcon {
  type Error = Error;
  fn try_from(icon: Icon<'_>) -> std::result::Result<Self, Self::Error> {
    TaoWindowIcon::from_rgba(icon.rgba.to_vec(), icon.width, icon.height)
      .map(Self)
      .map_err(|e| Error::InvalidIcon(Box::new(e)))
  }
}
