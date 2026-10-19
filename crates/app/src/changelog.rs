//! What's new: the released versions from `CHANGELOG.md`, compiled in and
//! shown in the app. Only the Markdown the changelog uses is understood:
//! version and section headings, nested lists with wrapped lines, **bold**,
//! `code` and [links](…).

use eframe::egui::{self, RichText, text::LayoutJob};

use crate::theme;

const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

/// One released version.
pub struct Release {
    /// "0.4.0".
    pub version: String,
    /// "2026-10-20", as written.
    pub date: String,
    pub sections: Vec<Section>,
}

pub struct Section {
    /// "Added", "Changed"…
    pub title: String,
    pub items: Vec<Item>,
}

pub struct Item {
    /// 0 for a top-level point, 1 for one under it…
    pub depth: usize,
    pub text: String,
}

/// The released versions, newest first (the unreleased notes left out).
pub fn releases() -> Vec<Release> {
    let mut releases: Vec<Release> = Vec::new();
    // Inside "## [Unreleased]" or before the first version: skip.
    let mut skipping = true;
    for line in CHANGELOG.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let (name, date) = heading.split_once(" - ").unwrap_or((heading, ""));
            let version = name.trim_matches(|c| c == '[' || c == ']').to_owned();
            skipping = version.eq_ignore_ascii_case("unreleased");
            if !skipping {
                releases.push(Release {
                    version,
                    date: date.trim().to_owned(),
                    sections: Vec::new(),
                });
            }
            continue;
        }
        let Some(release) = releases.last_mut().filter(|_| !skipping) else {
            continue;
        };
        // Link definitions at the end: "[0.4.0]: https://…".
        if line.starts_with('[') && line.contains("]: ") {
            continue;
        }
        if let Some(title) = line.strip_prefix("### ") {
            release.sections.push(Section {
                title: title.trim().to_owned(),
                items: Vec::new(),
            });
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if release.sections.is_empty() {
            release.sections.push(Section {
                title: String::new(),
                items: Vec::new(),
            });
        }
        let items = &mut release.sections.last_mut().expect("just made").items;
        if let Some(point) = trimmed.strip_prefix("- ") {
            items.push(Item {
                depth: indent / 2,
                text: point.to_owned(),
            });
        } else if let Some(item) = items.last_mut() {
            // A wrapped line, or a paragraph under a point.
            item.text.push(' ');
            item.text.push_str(trimmed);
        }
    }
    releases
}

/// The changelog, newest first (put it in a scroll area).
pub fn show(ui: &mut egui::Ui) {
    for release in releases() {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Version {}", release.version))
                    .heading()
                    .color(theme::BLAZE),
            );
            if release.version == crate::VERSION {
                ui.label(RichText::new("this version").small().color(theme::MEADOW));
            }
        });
        if !release.date.is_empty() {
            ui.weak(&release.date);
        }
        for section in &release.sections {
            if !section.title.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(&section.title).strong());
            }
            for item in &section.items {
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0 + 16.0 * item.depth as f32);
                    ui.label(if item.depth == 0 { "•" } else { "◦" });
                    ui.add(egui::Label::new(inline(ui, &item.text)).wrap());
                });
            }
        }
        ui.add_space(6.0);
        ui.separator();
    }
}

/// A line with **bold**, `code` and [links](…) as styled text.
fn inline(ui: &egui::Ui, text: &str) -> LayoutJob {
    let body = egui::TextStyle::Body.resolve(ui.style());
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    // Bold shows as the bright text colour against a quieter one.
    let plain = theme::BONE_MUTED;
    let mut job = LayoutJob::default();
    let mut bold = false;
    let mut rest = text;
    while !rest.is_empty() {
        let next = rest.find(['*', '`', '[']).unwrap_or(rest.len());
        let (before, after) = rest.split_at(next);
        let format = |bold: bool| {
            egui::TextFormat::simple(body.clone(), if bold { theme::BONE } else { plain })
        };
        if !before.is_empty() {
            job.append(before, 0.0, format(bold));
        }
        rest = after;
        if let Some(after) = rest.strip_prefix("**") {
            bold = !bold;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('`') {
            let end = after.find('`').unwrap_or(after.len());
            job.append(
                &after[..end],
                0.0,
                egui::TextFormat::simple(mono.clone(), plain),
            );
            rest = after.get(end + 1..).unwrap_or("");
        } else if let Some(after) = rest.strip_prefix('[') {
            // [text](url): show the text.
            match after.find("](").zip(after.find(')')) {
                Some((close, end)) if close < end => {
                    job.append(&after[..close], 0.0, format(bold));
                    rest = &after[end + 1..];
                }
                _ => {
                    job.append("[", 0.0, format(bold));
                    rest = after;
                }
            }
        } else if !rest.is_empty() {
            // A lone '*'.
            job.append(&rest[..1], 0.0, format(bold));
            rest = &rest[1..];
        }
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_version_is_in_the_changelog() {
        let releases = releases();
        assert_eq!(
            releases.first().map(|r| r.version.as_str()),
            Some(crate::VERSION),
            "CHANGELOG.md needs a heading for this version"
        );
        assert!(
            releases.iter().all(|r| !r.sections.is_empty()),
            "an empty version"
        );
        assert!(releases.iter().any(|r| r.version == "0.1.0"));
    }

    #[test]
    fn wrapped_lines_and_nested_points_are_read() {
        let release = releases()
            .into_iter()
            .find(|r| r.version == "0.2.0")
            .unwrap();
        let items: Vec<&Item> = release.sections.iter().flat_map(|s| &s.items).collect();
        assert!(items.iter().any(|item| item.depth == 1), "a nested point");
        assert!(
            items
                .iter()
                .any(|item| item.text.starts_with("**Phone layout:**")
                    && item.text.contains("touch targets")),
            "a point and its wrapped line"
        );
        assert!(items.iter().all(|item| !item.text.contains("]: http")));
    }
}
