//! CLDR cardinal plural rules, evaluated according to UTS #35.
//!
//! Decimal spelling matters: English `1` is `one`, while `1.0` is `other`.
//! Rules are evaluated in CLDR category order, with `other` as the fallback.

mod data;
mod operands;
mod rule;

pub use data::cldr_rules;
pub use operands::Operands;

/// A CLDR plural category, in semantic evaluation order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Category {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl Category {
    pub(super) const ALL: [Self; 6] = [
        Self::Zero,
        Self::One,
        Self::Two,
        Self::Few,
        Self::Many,
        Self::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Few => "few",
            Self::Many => "many",
            Self::Other => "other",
        }
    }

    /// Parse a lowercase CLDR category name.
    #[allow(clippy::should_implement_trait)] // Optional lookup alongside the FromStr implementation.
    pub fn from_str(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == text)
    }
}

impl std::str::FromStr for Category {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::from_str(text).ok_or_else(|| format!("unknown plural category: {text}"))
    }
}

/// Parsed cardinal rules for one locale; parsing is separate from selection.
#[derive(Clone, Debug)]
pub struct Rules {
    rules: Vec<(Category, rule::Condition)>,
}

impl Rules {
    /// Parse modern CLDR conditions, ignoring everything from the first `@`.
    ///
    /// Supports `%` / `mod`, `=` / `!=`, integer range lists, `and`, and `or`.
    /// Legacy `is`, `in`, `within`, and `not` relations are rejected.
    /// Rule integer literals and moduli must fit in u64.
    pub fn parse(rules: &[(Category, &str)]) -> Result<Self, String> {
        let mut parsed = Vec::new();
        let mut seen = Vec::new();
        for &(category, text) in rules {
            if seen.contains(&category) {
                return Err(format!("duplicate category: {}", category.as_str()));
            }
            seen.push(category);
            let condition = text
                .split('@')
                .next()
                .unwrap_or_default()
                .trim_matches(rule::pattern_space);
            if category == Category::Other {
                if !condition.is_empty() {
                    return Err("other must have an empty condition".into());
                }
            } else {
                parsed.push((category, rule::parse(condition)?));
            }
        }
        parsed.sort_by_key(|(category, _)| *category);
        Ok(Self { rules: parsed })
    }

    /// Select a category. NaN and either infinity always select `Other`.
    pub fn select(&self, operands: &Operands) -> Category {
        if !operands.finite {
            return Category::Other;
        }
        self.rules
            .iter()
            .find(|(_, condition)| rule::matches(condition, operands))
            .map_or(Category::Other, |(category, _)| *category)
    }

    /// Categories in CLDR order, always ending with `Other` exactly once.
    pub fn categories(&self) -> impl Iterator<Item = Category> + '_ {
        self.rules
            .iter()
            .map(|(category, _)| *category)
            .chain(std::iter::once(Category::Other))
    }
}

#[cfg(test)]
mod tests;
