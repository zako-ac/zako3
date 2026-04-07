use std::fmt;
use zako3_types::AudioRequestString;

/// A request identifier passed to a tap — typically a URL.
#[derive(Debug, Clone)]
pub struct AudioSource(String);

impl AudioSource {
    pub fn url(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AudioSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<AudioRequestString> for AudioSource {
    fn from(ars: AudioRequestString) -> Self {
        Self(ars.to_string())
    }
}
