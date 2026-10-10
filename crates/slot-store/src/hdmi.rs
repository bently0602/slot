#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum HdmiScale {
    #[default]
    Fit,
    Integer,
    Stretch,
}

impl HdmiScale {
    pub const ALL: [Self; 3] = [Self::Fit, Self::Integer, Self::Stretch];

    pub fn name(self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::Integer => "integer",
            Self::Stretch => "stretch",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == value)
    }

    pub fn step(self, right: bool) -> Self {
        let at = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(at + if right { 1 } else { 2 }) % 3]
    }
}
