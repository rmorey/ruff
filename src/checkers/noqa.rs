//! `NoQA` enforcement and validation.

use std::str::FromStr;

use nohash_hasher::IntMap;
use rustpython_parser::ast::Location;

use crate::ast::types::Range;
use crate::fix::Fix;
use crate::noqa::{is_file_exempt, Directive};
use crate::registry::{Diagnostic, DiagnosticKind, RuleCode, CODE_REDIRECTS};
use crate::settings::{flags, Settings};
use crate::violations::UnusedCodes;
use crate::{noqa, violations};

pub fn check_noqa(
    diagnostics: &mut Vec<Diagnostic>,
    contents: &str,
    commented_lines: &[usize],
    noqa_line_for: &IntMap<usize, usize>,
    settings: &Settings,
    autofix: flags::Autofix,
) {
    let mut noqa_directives: IntMap<usize, (Directive, Vec<&str>)> = IntMap::default();
    let mut ignored = vec![];

    let enforce_noqa = settings.rules.enabled(&RuleCode::RUF100);

    let lines: Vec<&str> = contents.lines().collect();
    for lineno in commented_lines {
        // If we hit an exemption for the entire file, bail.
        if is_file_exempt(lines[lineno - 1]) {
            diagnostics.drain(..);
            return;
        }

        if enforce_noqa {
            noqa_directives
                .entry(lineno - 1)
                .or_insert_with(|| (noqa::extract_noqa_directive(lines[lineno - 1]), vec![]));
        }
    }

    // Remove any ignored diagnostics.
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if matches!(diagnostic.kind, DiagnosticKind::BlanketNOQA(..)) {
            continue;
        }

        // Is the violation ignored by a `noqa` directive on the parent line?
        if let Some(parent_lineno) = diagnostic.parent.map(|location| location.row()) {
            let noqa_lineno = noqa_line_for.get(&parent_lineno).unwrap_or(&parent_lineno);
            if commented_lines.contains(noqa_lineno) {
                let noqa = noqa_directives.entry(noqa_lineno - 1).or_insert_with(|| {
                    (noqa::extract_noqa_directive(lines[noqa_lineno - 1]), vec![])
                });
                match noqa {
                    (Directive::All(..), matches) => {
                        matches.push(diagnostic.kind.code().as_ref());
                        ignored.push(index);
                        continue;
                    }
                    (Directive::Codes(.., codes), matches) => {
                        if noqa::includes(diagnostic.kind.code(), codes) {
                            matches.push(diagnostic.kind.code().as_ref());
                            ignored.push(index);
                            continue;
                        }
                    }
                    (Directive::None, ..) => {}
                }
            }
        }

        // Is the diagnostic ignored by a `noqa` directive on the same line?
        let diagnostic_lineno = diagnostic.location.row();
        let noqa_lineno = noqa_line_for
            .get(&diagnostic_lineno)
            .unwrap_or(&diagnostic_lineno);
        if commented_lines.contains(noqa_lineno) {
            let noqa = noqa_directives
                .entry(noqa_lineno - 1)
                .or_insert_with(|| (noqa::extract_noqa_directive(lines[noqa_lineno - 1]), vec![]));
            match noqa {
                (Directive::All(..), matches) => {
                    matches.push(diagnostic.kind.code().as_ref());
                    ignored.push(index);
                }
                (Directive::Codes(.., codes), matches) => {
                    if noqa::includes(diagnostic.kind.code(), codes) {
                        matches.push(diagnostic.kind.code().as_ref());
                        ignored.push(index);
                    }
                }
                (Directive::None, ..) => {}
            }
        }
    }

    // Enforce that the noqa directive was actually used (RUF100).
    if enforce_noqa {
        for (row, (directive, matches)) in noqa_directives {
            match directive {
                Directive::All(spaces, start, end) => {
                    if matches.is_empty() {
                        let mut diagnostic = Diagnostic::new(
                            violations::UnusedNOQA(None),
                            Range::new(Location::new(row + 1, start), Location::new(row + 1, end)),
                        );
                        if matches!(autofix, flags::Autofix::Enabled)
                            && settings.rules.should_fix(diagnostic.kind.code())
                        {
                            diagnostic.amend(Fix::deletion(
                                Location::new(row + 1, start - spaces),
                                Location::new(row + 1, lines[row].chars().count()),
                            ));
                        }
                        diagnostics.push(diagnostic);
                    }
                }
                Directive::Codes(spaces, start, end, codes) => {
                    let mut disabled_codes = vec![];
                    let mut unknown_codes = vec![];
                    let mut unmatched_codes = vec![];
                    let mut valid_codes = vec![];
                    let mut self_ignore = false;
                    for code in codes {
                        let code = CODE_REDIRECTS.get(code).map_or(code, AsRef::as_ref);
                        if code == RuleCode::RUF100.as_ref() {
                            self_ignore = true;
                            break;
                        }

                        if matches.contains(&code) || settings.external.contains(code) {
                            valid_codes.push(code);
                        } else {
                            if let Ok(rule_code) = RuleCode::from_str(code) {
                                if settings.rules.enabled(&rule_code) {
                                    unmatched_codes.push(code);
                                } else {
                                    disabled_codes.push(code);
                                }
                            } else {
                                unknown_codes.push(code);
                            }
                        }
                    }

                    if self_ignore {
                        continue;
                    }

                    if !(disabled_codes.is_empty()
                        && unknown_codes.is_empty()
                        && unmatched_codes.is_empty())
                    {
                        let mut diagnostic = Diagnostic::new(
                            violations::UnusedNOQA(Some(UnusedCodes {
                                disabled: disabled_codes
                                    .iter()
                                    .map(|code| (*code).to_string())
                                    .collect(),
                                unknown: unknown_codes
                                    .iter()
                                    .map(|code| (*code).to_string())
                                    .collect(),
                                unmatched: unmatched_codes
                                    .iter()
                                    .map(|code| (*code).to_string())
                                    .collect(),
                            })),
                            Range::new(Location::new(row + 1, start), Location::new(row + 1, end)),
                        );
                        if matches!(autofix, flags::Autofix::Enabled)
                            && settings.rules.should_fix(diagnostic.kind.code())
                        {
                            if valid_codes.is_empty() {
                                diagnostic.amend(Fix::deletion(
                                    Location::new(row + 1, start - spaces),
                                    Location::new(row + 1, lines[row].chars().count()),
                                ));
                            } else {
                                diagnostic.amend(Fix::replacement(
                                    format!("# noqa: {}", valid_codes.join(", ")),
                                    Location::new(row + 1, start),
                                    Location::new(row + 1, lines[row].chars().count()),
                                ));
                            }
                        }
                        diagnostics.push(diagnostic);
                    }
                }
                Directive::None => {}
            }
        }
    }

    ignored.sort_unstable();
    for index in ignored.iter().rev() {
        diagnostics.swap_remove(*index);
    }
}
