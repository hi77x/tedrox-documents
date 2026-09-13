//! Page selection parsing: `1-5,8,10-`, `odd`, `even`, `all`.

use tdx_core::error::{Result, TdxError};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Single(u32),
    Range(u32, Option<u32>),
    Odd,
    Even,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageSelection {
    parts: Vec<Part>,
}

impl PageSelection {
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(TdxError::InvalidInput("Empty page selection".into()));
        }
        if trimmed.eq_ignore_ascii_case("all") {
            return Ok(Self { parts: vec![] });
        }
        let mut parts = Vec::new();
        for raw in trimmed.split(',') {
            let token = raw.trim();
            if token.is_empty() {
                continue;
            }
            let lower = token.to_ascii_lowercase();
            match lower.as_str() {
                "odd" => {
                    parts.push(Part::Odd);
                    continue;
                }
                "even" => {
                    parts.push(Part::Even);
                    continue;
                }
                _ => {}
            }
            if let Some((start, end)) = token.split_once('-') {
                let start: u32 = start
                    .trim()
                    .parse()
                    .map_err(|_| TdxError::InvalidInput(format!("Invalid page range: {token}")))?;
                if start == 0 {
                    return Err(TdxError::InvalidInput("Page numbers start at 1".into()));
                }
                let end = end.trim();
                let end = if end.is_empty() {
                    None
                } else {
                    let value: u32 = end.parse().map_err(|_| {
                        TdxError::InvalidInput(format!("Invalid page range: {token}"))
                    })?;
                    if value < start {
                        return Err(TdxError::InvalidInput(format!(
                            "Page range is reversed: {token}"
                        )));
                    }
                    Some(value)
                };
                parts.push(Part::Range(start, end));
            } else {
                let page: u32 = token
                    .parse()
                    .map_err(|_| TdxError::InvalidInput(format!("Invalid page number: {token}")))?;
                if page == 0 {
                    return Err(TdxError::InvalidInput("Page numbers start at 1".into()));
                }
                parts.push(Part::Single(page));
            }
        }
        if parts.is_empty() {
            return Err(TdxError::InvalidInput("Empty page selection".into()));
        }
        Ok(Self { parts })
    }

    pub fn all() -> Self {
        Self { parts: vec![] }
    }

    pub fn is_all(&self) -> bool {
        self.parts.is_empty()
    }

    /// Expand to sorted unique 1-based page numbers.
    pub fn resolve(&self, page_count: u32) -> Result<Vec<u32>> {
        if page_count == 0 {
            return Err(TdxError::Corrupt("The document has no pages".into()));
        }
        if self.is_all() {
            return Ok((1..=page_count).collect());
        }
        let mut pages: Vec<u32> = Vec::new();
        for part in &self.parts {
            match part {
                Part::Single(page) => {
                    if *page > page_count {
                        return Err(TdxError::InvalidInput(format!(
                            "Page {page} is outside this document (1-{page_count})"
                        )));
                    }
                    pages.push(*page);
                }
                Part::Range(start, end) => {
                    let end = end.unwrap_or(page_count);
                    if *start > page_count {
                        return Err(TdxError::InvalidInput(format!(
                            "Page {start} is outside this document (1-{page_count})"
                        )));
                    }
                    let end = end.min(page_count);
                    pages.extend(*start..=end);
                }
                Part::Odd => pages.extend((1..=page_count).filter(|p| p % 2 == 1)),
                Part::Even => pages.extend((1..=page_count).filter(|p| p % 2 == 0)),
            }
        }
        pages.sort_unstable();
        pages.dedup();
        if pages.is_empty() {
            return Err(TdxError::InvalidInput(
                "The page selection is empty for this document".into(),
            ));
        }
        Ok(pages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all() {
        assert!(PageSelection::parse("ALL").unwrap().is_all());
        assert_eq!(PageSelection::all().resolve(3).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn parses_ranges() {
        let selection = PageSelection::parse("1-3,7,10-").unwrap();
        assert_eq!(selection.resolve(12).unwrap(), vec![1, 2, 3, 7, 10, 11, 12]);
    }

    #[test]
    fn parses_odd_even() {
        assert_eq!(
            PageSelection::parse("odd").unwrap().resolve(5).unwrap(),
            vec![1, 3, 5]
        );
        assert_eq!(
            PageSelection::parse("even").unwrap().resolve(5).unwrap(),
            vec![2, 4]
        );
    }

    #[test]
    fn rejects_out_of_range() {
        let selection = PageSelection::parse("9").unwrap();
        assert!(selection.resolve(5).is_err());
    }

    #[test]
    fn rejects_reversed_range() {
        assert!(PageSelection::parse("5-2").is_err());
    }
}
