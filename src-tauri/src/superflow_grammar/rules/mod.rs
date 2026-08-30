use harper_core::{
    expr::{Expr, SequenceExpr},
    linting::{Chunk, ExprLinter, Lint, LintGroup, LintKind, Suggestion},
    Span, Token,
};

type Alternatives = &'static [&'static str];

struct WordReplacementRule {
    expr: SequenceExpr,
    target_token: usize,
    replacements: &'static [(&'static str, &'static str)],
    description: &'static str,
}

impl WordReplacementRule {
    fn new(
        parts: &'static [Alternatives],
        target_word: usize,
        replacements: &'static [(&'static str, &'static str)],
        description: &'static str,
    ) -> Self {
        let mut expr = SequenceExpr::default().then_word_set(parts[0]);
        for part in &parts[1..] {
            expr = expr.t_ws().then_word_set(part);
        }
        Self {
            expr,
            target_token: target_word * 2,
            replacements,
            description,
        }
    }
}

impl ExprLinter for WordReplacementRule {
    type Unit = Chunk;

    fn expr(&self) -> &dyn Expr {
        &self.expr
    }

    fn match_to_lint(&self, tokens: &[Token], source: &[char]) -> Option<Lint> {
        let token = tokens.get(self.target_token)?;
        let original = token.span.get_content(source);
        let normalized = original.iter().collect::<String>().to_lowercase();
        let replacement = self
            .replacements
            .iter()
            .find_map(|(from, to)| (*from == normalized).then_some(*to))?;
        Some(Lint {
            span: token.span,
            lint_kind: LintKind::Grammar,
            suggestions: vec![Suggestion::replace_with_match_case(
                replacement.chars().collect(),
                original,
            )],
            message: self.description.into(),
            priority: 20,
        })
    }

    fn description(&self) -> &str {
        self.description
    }
}

struct CompoundModifierRule {
    expr: SequenceExpr,
    replacement: &'static str,
    modifier_words: usize,
}

impl CompoundModifierRule {
    fn new(modifier: Alternatives, nouns: Alternatives, replacement: &'static str) -> Self {
        let mut expr = SequenceExpr::default().t_aco(modifier[0]);
        for word in &modifier[1..] {
            expr = expr.t_ws().t_aco(word);
        }
        Self {
            expr: expr.t_ws().then_word_set(nouns),
            replacement,
            modifier_words: modifier.len(),
        }
    }
}

impl ExprLinter for CompoundModifierRule {
    type Unit = Chunk;

    fn expr(&self) -> &dyn Expr {
        &self.expr
    }

    fn match_to_lint(&self, tokens: &[Token], source: &[char]) -> Option<Lint> {
        let last_modifier = (self.modifier_words - 1) * 2;
        let span = Span::new(
            tokens.first()?.span.start,
            tokens.get(last_modifier)?.span.end,
        );
        let original = span.get_content(source);
        Some(Lint {
            span,
            lint_kind: LintKind::Grammar,
            suggestions: vec![Suggestion::replace_with_match_case(
                self.replacement.chars().collect(),
                original,
            )],
            message: "Hyphenate an established compound modifier before a noun.".into(),
            priority: 30,
        })
    }

    fn description(&self) -> &str {
        "Hyphenates an established compound modifier only before a supported noun."
    }
}

fn add(
    group: &mut LintGroup,
    name: &'static str,
    parts: &'static [Alternatives],
    target_word: usize,
    replacements: &'static [(&'static str, &'static str)],
    description: &'static str,
) {
    assert!(group.add_chunk_expr_linter(
        name,
        WordReplacementRule::new(parts, target_word, replacements, description)
    ));
    group.config.set_rule_enabled(name, true);
}

const SINGULAR_PRONOUN: Alternatives = &["he", "she", "it"];
const PLURAL_PRONOUN: Alternatives = &["i", "you", "we", "they"];
const PERFECT_AUX: Alternatives = &["have", "has", "had"];
const PLURAL_MARKER: Alternatives = &[
    "many", "several", "multiple", "numerous", "few", "two", "three", "four", "five", "six",
    "seven", "eight", "nine", "ten",
];
const COMMON_SINGULAR: Alternatives = &[
    "issue", "problem", "bug", "error", "change", "test", "file", "item", "task", "day", "week",
    "month", "user", "record", "request", "message", "meeting", "feature",
];
const COMMON_PLURALS: &[(&str, &str)] = &[
    ("issue", "issues"),
    ("problem", "problems"),
    ("bug", "bugs"),
    ("error", "errors"),
    ("change", "changes"),
    ("test", "tests"),
    ("file", "files"),
    ("item", "items"),
    ("task", "tasks"),
    ("day", "days"),
    ("week", "weeks"),
    ("month", "months"),
    ("user", "users"),
    ("record", "records"),
    ("request", "requests"),
    ("message", "messages"),
    ("meeting", "meetings"),
    ("feature", "features"),
];

pub fn register(group: &mut LintGroup) {
    add(
        group,
        "SfContractions",
        &[&[
            "cant", "wont", "isnt", "arent", "wasnt", "werent", "hasnt", "havent", "hadnt",
            "didnt", "doesnt", "couldnt", "shouldnt", "wouldnt",
        ]],
        0,
        &[
            ("cant", "can't"),
            ("wont", "won't"),
            ("isnt", "isn't"),
            ("arent", "aren't"),
            ("wasnt", "wasn't"),
            ("werent", "weren't"),
            ("hasnt", "hasn't"),
            ("havent", "haven't"),
            ("hadnt", "hadn't"),
            ("didnt", "didn't"),
            ("doesnt", "doesn't"),
            ("couldnt", "couldn't"),
            ("shouldnt", "shouldn't"),
            ("wouldnt", "wouldn't"),
        ],
        "Restore an unambiguous missing contraction apostrophe.",
    );
    add(
        group,
        "SfDontApostrophe",
        &[&["dont"]],
        0,
        &[("dont", "don't")],
        "Restore the missing apostrophe in don't.",
    );
    add(
        group,
        "SfSingularHave",
        &[SINGULAR_PRONOUN, &["have"]],
        1,
        &[("have", "has")],
        "Use singular verb agreement.",
    );
    add(
        group,
        "SfSingularDo",
        &[SINGULAR_PRONOUN, &["do"]],
        1,
        &[("do", "does")],
        "Use singular verb agreement.",
    );
    add(
        group,
        "SfSingularGo",
        &[SINGULAR_PRONOUN, &["go"]],
        1,
        &[("go", "goes")],
        "Use singular verb agreement.",
    );
    add(
        group,
        "SfPluralHas",
        &[PLURAL_PRONOUN, &["has"]],
        1,
        &[("has", "have")],
        "Use plural verb agreement.",
    );
    add(
        group,
        "SfPluralDoes",
        &[PLURAL_PRONOUN, &["does"]],
        1,
        &[("does", "do")],
        "Use plural verb agreement.",
    );
    add(
        group,
        "SfPluralGoes",
        &[PLURAL_PRONOUN, &["goes"]],
        1,
        &[("goes", "go")],
        "Use plural verb agreement.",
    );
    add(
        group,
        "SfSingularDemonstrativePresent",
        &[&["this"], &["are"]],
        1,
        &[("are", "is")],
        "Match a singular demonstrative with a singular verb.",
    );
    add(
        group,
        "SfSingularDemonstrativePast",
        &[&["this"], &["were"]],
        1,
        &[("were", "was")],
        "Match a singular demonstrative with a singular verb.",
    );
    add(
        group,
        "SfPluralDemonstrativePresent",
        &[&["these", "those"], &["is"]],
        1,
        &[("is", "are")],
        "Match a plural demonstrative with a plural verb.",
    );
    add(
        group,
        "SfPluralDemonstrativePast",
        &[&["these", "those"], &["was"]],
        1,
        &[("was", "were")],
        "Match a plural demonstrative with a plural verb.",
    );
    add(
        group,
        "SfThereIsPlural",
        &[&["there"], &["is"], PLURAL_MARKER],
        1,
        &[("is", "are")],
        "Use a plural existential verb before explicit plural evidence.",
    );
    add(
        group,
        "SfThereWasPlural",
        &[&["there"], &["was"], PLURAL_MARKER],
        1,
        &[("was", "were")],
        "Use a plural existential verb before explicit plural evidence.",
    );
    add(
        group,
        "SfPerfectWent",
        &[PERFECT_AUX, &["went"]],
        1,
        &[("went", "gone")],
        "Use the past participle after a perfect auxiliary.",
    );
    add(
        group,
        "SfPerfectWrote",
        &[PERFECT_AUX, &["wrote"]],
        1,
        &[("wrote", "written")],
        "Use the past participle after a perfect auxiliary.",
    );
    add(
        group,
        "SfPerfectTook",
        &[PERFECT_AUX, &["took"]],
        1,
        &[("took", "taken")],
        "Use the past participle after a perfect auxiliary.",
    );
    add(
        group,
        "SfPerfectGave",
        &[PERFECT_AUX, &["gave"]],
        1,
        &[("gave", "given")],
        "Use the past participle after a perfect auxiliary.",
    );
    add(
        group,
        "SfPerfectDrove",
        &[PERFECT_AUX, &["drove"]],
        1,
        &[("drove", "driven")],
        "Use the past participle after a perfect auxiliary.",
    );
    add(
        group,
        "SfPerfectAte",
        &[PERFECT_AUX, &["ate"]],
        1,
        &[("ate", "eaten")],
        "Use the past participle after a perfect auxiliary.",
    );
    add(
        group,
        "SfPerfectBroke",
        &[PERFECT_AUX, &["broke"]],
        1,
        &[("broke", "broken")],
        "Use the past participle after a perfect auxiliary.",
    );
    add(
        group,
        "SfFirstPersonSeen",
        &[&["i", "we"], &["seen"]],
        1,
        &[("seen", "saw")],
        "Use the simple past without a perfect auxiliary.",
    );
    add(
        group,
        "SfFirstPersonDone",
        &[&["i", "we"], &["done"]],
        1,
        &[("done", "did")],
        "Use the simple past without a perfect auxiliary.",
    );
    add(
        group,
        "SfDidPast",
        &[
            &["did"],
            &[
                "went", "saw", "wrote", "took", "gave", "drove", "ate", "broke",
            ],
        ],
        1,
        &[
            ("went", "go"),
            ("saw", "see"),
            ("wrote", "write"),
            ("took", "take"),
            ("gave", "give"),
            ("drove", "drive"),
            ("ate", "eat"),
            ("broke", "break"),
        ],
        "Use the base verb after did.",
    );
    add(
        group,
        "SfQuantifierPlural",
        &[PLURAL_MARKER, COMMON_SINGULAR],
        1,
        COMMON_PLURALS,
        "Use a plural count noun after explicit plural evidence.",
    );
    add(
        group,
        "SfCoupleOfPlural",
        &[&["a"], &["couple"], &["of"], COMMON_SINGULAR],
        3,
        COMMON_PLURALS,
        "Use a plural count noun after a couple of.",
    );
    add(
        group,
        "SfSeeingPlural",
        &[&["seeing"], COMMON_SINGULAR],
        1,
        COMMON_PLURALS,
        "Use a plural count noun after seeing when no determiner is present.",
    );
    const SUBJECT: Alternatives = &[
        "i", "you", "he", "she", "we", "they", "me", "him", "her", "us", "them", "sam", "alex",
        "sarah",
    ];
    add(
        group,
        "SfCompoundSubjectWas",
        &[SUBJECT, &["and"], SUBJECT, &["was"]],
        3,
        &[("was", "were")],
        "Use a plural verb with a compound subject.",
    );
    add(
        group,
        "SfCompoundSubjectHas",
        &[SUBJECT, &["and"], SUBJECT, &["has"]],
        3,
        &[("has", "have")],
        "Use a plural verb with a compound subject.",
    );

    for (name, modifier, nouns, replacement) in [
        (
            "SfLongTermModifier",
            &["long", "term"] as Alternatives,
            &["plan", "strategy", "goal", "solution", "impact"] as Alternatives,
            "long-term",
        ),
        (
            "SfWellKnownModifier",
            &["well", "known"],
            &["bug", "issue", "problem", "fact", "limitation"],
            "well-known",
        ),
        (
            "SfOpenSourceModifier",
            &["open", "source"],
            &["project", "library", "software", "tool", "model"],
            "open-source",
        ),
        (
            "SfPrivacyFirstModifier",
            &["privacy", "first"],
            &["design", "product", "architecture", "system", "approach"],
            "privacy-first",
        ),
        (
            "SfStateOfTheArtModifier",
            &["state", "of", "the", "art"],
            &["system", "model", "design", "method", "solution"],
            "state-of-the-art",
        ),
    ] {
        assert!(group.add_chunk_expr_linter(
            name,
            CompoundModifierRule::new(modifier, nouns, replacement)
        ));
        group.config.set_rule_enabled(name, true);
    }
}

#[cfg(test)]
mod tests {
    use crate::superflow_grammar::correct;

    #[test]
    fn fixes_high_frequency_agreement_families() {
        let cases = [
            ("he have a plan", "he has a plan"),
            ("she do the review", "she does the review"),
            ("it go live", "it goes live"),
            ("they has a plan", "they have a plan"),
            ("we does the review", "we do the review"),
            ("you goes first", "you go first"),
            ("this are ready", "this is ready"),
            ("this were blocked", "this was blocked"),
            ("these is ready", "these are ready"),
            ("those was blocked", "those were blocked"),
            ("there is many problems", "there are many problems"),
            ("there was several issues", "there were several issues"),
            ("me and him was reviewing", "me and him were reviewing"),
            ("Sam and she has reviewed", "Sam and she have reviewed"),
        ];
        for (input, expected) in cases {
            assert_eq!(correct(input), expected, "input: {input}");
        }
    }

    #[test]
    fn fixes_participles_and_explicit_plural_evidence() {
        let cases = [
            ("we have went home", "we have gone home"),
            ("she has wrote it", "she has written it"),
            ("they had took it", "they had taken it"),
            ("he has gave notice", "he has given notice"),
            ("we have drove there", "we have driven there"),
            ("I had ate already", "I had eaten already"),
            ("it has broke again", "it has broken again"),
            ("I seen it yesterday", "I saw it yesterday"),
            ("we done it yesterday", "we did it yesterday"),
            ("did went home", "did go home"),
            ("many issue remain", "many issues remain"),
            ("a couple of problem remain", "a couple of problems remain"),
        ];
        for (input, expected) in cases {
            assert_eq!(correct(input), expected, "input: {input}");
        }
    }

    #[test]
    fn restores_unambiguous_contractions() {
        for (input, expected) in [
            ("we dont agree", "we don't agree"),
            ("we cant ship", "we can't ship"),
            ("it isnt ready", "it isn't ready"),
            ("they havent replied", "they haven't replied"),
            ("we shouldnt guess", "we shouldn't guess"),
        ] {
            assert_eq!(correct(input), expected, "input: {input}");
        }
    }

    #[test]
    fn hyphenates_only_attributive_compounds() {
        for (input, expected) in [
            ("a long term plan", "a long-term plan"),
            ("the well known issue", "the well-known issue"),
            ("an open source project", "an open-source project"),
            ("a privacy first design", "a privacy-first design"),
            ("a state of the art system", "a state-of-the-art system"),
        ] {
            assert_eq!(correct(input), expected, "input: {input}");
        }
        for untouched in [
            "the plan is long term",
            "the issue is well known",
            "the project is open source",
            "privacy first is our principle",
            "there is one problem",
            "rolling out but afternoon",
            "the things that have changed",
        ] {
            assert_eq!(correct(untouched), untouched, "input: {untouched}");
        }
    }

    #[test]
    fn protected_content_is_immutable_across_custom_rules() {
        let output = correct(
            "many issue in src-tauri/src/foo.rs affect getUserById at /api/parse and foo@bar.com",
        );
        assert!(output.starts_with("many issues"));
        for token in [
            "src-tauri/src/foo.rs",
            "getUserById",
            "/api/parse",
            "foo@bar.com",
        ] {
            assert!(output.contains(token), "missing {token}: {output}");
        }
    }
}
