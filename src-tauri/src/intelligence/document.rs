#[derive(Debug, Clone, PartialEq, Eq)]
enum Block {
    Heading { level: u8, text: String },
    Paragraph(String),
    Bullet(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Document {
    blocks: Vec<Block>,
}

impl Document {
    fn render(&self) -> String {
        let mut output = String::new();
        let mut previous_was_bullet = false;
        for block in &self.blocks {
            let is_bullet = matches!(block, Block::Bullet(_));
            if !(output.is_empty() || is_bullet && previous_was_bullet) {
                output.push_str("\n\n");
            } else if !output.is_empty() {
                output.push('\n');
            }
            match block {
                Block::Heading { level, text } => {
                    output.push_str(&"#".repeat(usize::from((*level).clamp(1, 2))));
                    output.push(' ');
                    output.push_str(text.trim());
                }
                Block::Paragraph(text) => output.push_str(text.trim()),
                Block::Bullet(text) => {
                    output.push_str("- ");
                    output.push_str(text.trim());
                }
            }
            previous_was_bullet = is_bullet;
        }
        output
    }
}

pub(crate) fn structure_developer_text(text: &str) -> String {
    let sentences = sentences(text);
    if sentences.is_empty() || text.trim_start().starts_with('#') {
        return text.trim().to_string();
    }

    let mut document = Document::default();
    document.blocks.push(Block::Heading {
        level: 1,
        text: "Task".into(),
    });
    document
        .blocks
        .push(Block::Paragraph(sentences[0].to_string()));

    let mut check = Vec::new();
    let mut requirements = Vec::new();
    let mut context = Vec::new();
    for sentence in sentences.into_iter().skip(1) {
        let lower = sentence.to_ascii_lowercase();
        if starts_with_any(&lower, &["check ", "inspect ", "review ", "verify "]) {
            check.push(sentence.to_string());
        } else if starts_with_any(
            &lower,
            &[
                "do not ",
                "don't ",
                "never ",
                "keep ",
                "preserve ",
                "avoid ",
                "only ",
                "make ",
                "return ",
                "handle ",
            ],
        ) {
            requirements.push(sentence.to_string());
        } else {
            context.push(sentence.to_string());
        }
    }

    push_section(&mut document, "Context", context, false);
    push_section(&mut document, "Check", check, true);
    push_section(&mut document, "Requirements", requirements, true);
    document.render()
}

fn push_section(document: &mut Document, heading: &str, items: Vec<String>, bullets: bool) {
    if items.is_empty() {
        return;
    }
    document.blocks.push(Block::Heading {
        level: 2,
        text: heading.into(),
    });
    document.blocks.extend(items.into_iter().map(|item| {
        if bullets {
            Block::Bullet(item)
        } else {
            Block::Paragraph(item)
        }
    }));
}

fn starts_with_any(text: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| text.starts_with(prefix))
}

fn sentences(text: &str) -> Vec<&str> {
    let mut output = Vec::new();
    let mut start = 0usize;
    for (index, character) in text.char_indices() {
        if matches!(character, '.' | '!' | '?')
            && text[index + character.len_utf8()..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
        {
            let sentence = text[start..index + character.len_utf8()].trim();
            if !sentence.is_empty() {
                output.push(sentence);
            }
            start = index + character.len_utf8();
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        output.push(tail);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_adaptive_markdown_without_rewriting_sentences() {
        let output = structure_developer_text(
            "Fix the backend payment issue in `src/payment.ts`. Check the handler. Check validation. Do not change unrelated files. Preserve existing behavior.",
        );
        assert_eq!(
            output,
            "# Task\n\nFix the backend payment issue in `src/payment.ts`.\n\n## Check\n\n- Check the handler.\n- Check validation.\n\n## Requirements\n\n- Do not change unrelated files.\n- Preserve existing behavior."
        );
    }

    #[test]
    fn one_sentence_remains_faithful() {
        assert_eq!(
            structure_developer_text("Fix the payment handler."),
            "# Task\n\nFix the payment handler."
        );
    }

    #[test]
    fn existing_markdown_is_not_rewrapped() {
        let markdown = "# Task\n\nFix the handler.";
        assert_eq!(structure_developer_text(markdown), markdown);
    }
}
