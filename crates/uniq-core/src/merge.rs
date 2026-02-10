use serde::{Deserialize, Serialize};

use crate::variant::VariantId;

/// How much of a particular variant's technique to integrate in a merge.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlendRatio {
    /// 0% — not included, but the merge is "informed by" this variant's approach.
    Zero,
    /// 25% — contributes a minor sub-component.
    Quarter,
    /// 50% — equal architectural weight in the hybrid.
    Half,
    /// 75% — primary architecture, other side contributes sub-component.
    ThreeQuarter,
    /// 100% — full technique, other side only informs design decisions.
    Full,
}

impl BlendRatio {
    /// Get the numeric percentage.
    pub fn as_percent(&self) -> u8 {
        match self {
            BlendRatio::Zero => 0,
            BlendRatio::Quarter => 25,
            BlendRatio::Half => 50,
            BlendRatio::ThreeQuarter => 75,
            BlendRatio::Full => 100,
        }
    }

    /// Describe the blend for use in LLM prompts.
    pub fn description(&self) -> &'static str {
        match self {
            BlendRatio::Zero => "not directly used, but its insights inform the design",
            BlendRatio::Quarter => "contributes a minor sub-component or enhancement",
            BlendRatio::Half => "equal architectural weight in a true hybrid approach",
            BlendRatio::ThreeQuarter => "serves as the primary architecture",
            BlendRatio::Full => "fully implemented as the core approach",
        }
    }

    /// Get all blend ratios for UI display.
    pub fn all() -> &'static [BlendRatio] {
        &[
            BlendRatio::Zero,
            BlendRatio::Quarter,
            BlendRatio::Half,
            BlendRatio::ThreeQuarter,
            BlendRatio::Full,
        ]
    }

    /// Cycle to the next blend ratio (for keyboard navigation).
    pub fn next(&self) -> BlendRatio {
        match self {
            BlendRatio::Zero => BlendRatio::Quarter,
            BlendRatio::Quarter => BlendRatio::Half,
            BlendRatio::Half => BlendRatio::ThreeQuarter,
            BlendRatio::ThreeQuarter => BlendRatio::Full,
            BlendRatio::Full => BlendRatio::Full,
        }
    }

    /// Cycle to the previous blend ratio.
    pub fn prev(&self) -> BlendRatio {
        match self {
            BlendRatio::Zero => BlendRatio::Zero,
            BlendRatio::Quarter => BlendRatio::Zero,
            BlendRatio::Half => BlendRatio::Quarter,
            BlendRatio::ThreeQuarter => BlendRatio::Half,
            BlendRatio::Full => BlendRatio::ThreeQuarter,
        }
    }
}

impl std::fmt::Display for BlendRatio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.as_percent())
    }
}

/// Specification for merging two variants together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeSpec {
    /// First parent variant.
    pub parent_a: VariantId,

    /// Second parent variant.
    pub parent_b: VariantId,

    /// How much of variant A's technique to integrate.
    pub blend_a: BlendRatio,

    /// How much of variant B's technique to integrate.
    pub blend_b: BlendRatio,
}

impl MergeSpec {
    pub fn new(
        parent_a: VariantId,
        parent_b: VariantId,
        blend_a: BlendRatio,
        blend_b: BlendRatio,
    ) -> Self {
        Self {
            parent_a,
            parent_b,
            blend_a,
            blend_b,
        }
    }

    /// Generate a human-readable summary for display.
    pub fn summary(&self) -> String {
        format!(
            "{} ({}%) + {} ({}%)",
            self.parent_a,
            self.blend_a.as_percent(),
            self.parent_b,
            self.blend_b.as_percent(),
        )
    }
}

/// A node in the merge lineage tree, used for visualizing how merged variants
/// trace back to their original research techniques.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LineageNode {
    /// A leaf node — an original research-based variant.
    Original {
        variant_id: VariantId,
        technique_name: String,
        paper_id: String,
    },
    /// A merge node — has two parent lineage nodes.
    Merged {
        variant_id: VariantId,
        blend_a: BlendRatio,
        blend_b: BlendRatio,
        parent_a: Box<LineageNode>,
        parent_b: Box<LineageNode>,
    },
}

impl LineageNode {
    pub fn variant_id(&self) -> &VariantId {
        match self {
            LineageNode::Original { variant_id, .. } => variant_id,
            LineageNode::Merged { variant_id, .. } => variant_id,
        }
    }
}
