use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Accessibility,
    AdultSexualContent,
    ChildSexualExploitation,
    GraphicViolence,
    SelfHarm,
    HateOrHarassment,
    Extremism,
    IllegalActivities,
    Weapons,
    Drugs,
    Gambling,
    ScamsAndFraud,
    Misinformation,
    Profanity,
}

impl Category {
    pub const ALL: [Self; 14] = [
        Self::Accessibility,
        Self::AdultSexualContent,
        Self::ChildSexualExploitation,
        Self::GraphicViolence,
        Self::SelfHarm,
        Self::HateOrHarassment,
        Self::Extremism,
        Self::IllegalActivities,
        Self::Weapons,
        Self::Drugs,
        Self::Gambling,
        Self::ScamsAndFraud,
        Self::Misinformation,
        Self::Profanity,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScanStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScanPhase {
    Queued,
    Loading,
    Fetching,
    Accessibility,
    ContentSafety,
    Aggregating,
    Persisting,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FindingKind {
    Accessibility,
    ContentSafety,
}

macro_rules! impl_text_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value,)+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = InvalidEnumValue;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(InvalidEnumValue {
                        enum_name: stringify!($name),
                        value: value.to_owned(),
                    }),
                }
            }
        }
    };
}

impl_text_enum!(Severity {
    Low => "low",
    Medium => "medium",
    High => "high",
    Critical => "critical",
});

impl_text_enum!(Category {
    Accessibility => "accessibility",
    AdultSexualContent => "adult_sexual_content",
    ChildSexualExploitation => "child_sexual_exploitation",
    GraphicViolence => "graphic_violence",
    SelfHarm => "self_harm",
    HateOrHarassment => "hate_or_harassment",
    Extremism => "extremism",
    IllegalActivities => "illegal_activities",
    Weapons => "weapons",
    Drugs => "drugs",
    Gambling => "gambling",
    ScamsAndFraud => "scams_and_fraud",
    Misinformation => "misinformation",
    Profanity => "profanity",
});

impl_text_enum!(RiskLevel {
    Low => "low",
    Medium => "medium",
    High => "high",
    Critical => "critical",
});

impl_text_enum!(ScanStatus {
    Pending => "pending",
    Running => "running",
    Completed => "completed",
    Failed => "failed",
});

impl_text_enum!(ScanPhase {
    Queued => "queued",
    Loading => "loading",
    Fetching => "fetching",
    Accessibility => "accessibility",
    ContentSafety => "content_safety",
    Aggregating => "aggregating",
    Persisting => "persisting",
    Completed => "completed",
    Failed => "failed",
});

impl_text_enum!(FindingKind {
    Accessibility => "accessibility",
    ContentSafety => "content_safety",
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidEnumValue {
    pub enum_name: &'static str,
    pub value: String,
}

impl std::fmt::Display for InvalidEnumValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid {} value '{}'", self.enum_name, self.value)
    }
}

impl std::error::Error for InvalidEnumValue {}
