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
