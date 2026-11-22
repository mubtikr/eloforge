pub const MATCH_COACH: &str = include_str!("../../prompts/match_coach.md");
pub const REFLECTION_COACH: &str = include_str!("../../prompts/reflection_coach.md");
pub const WEEKLY_BRIEFING: &str = include_str!("../../prompts/weekly_briefing.md");
pub const CURRICULUM_NEXT: &str = include_str!("../../prompts/curriculum_next.md");

pub fn render(template: &str, values: &[(&str, String)]) -> String {
    let mut output = template.to_string();
    for (key, value) in values {
        output = output.replace(&format!("{{{{{key}}}}}"), value);
    }
    output
}

pub fn reflection_prompt(facts: &str, reflection: &str, memories: &str) -> String {
    render(
        REFLECTION_COACH,
        &[
            ("facts", facts.to_string()),
            ("reflection", reflection.to_string()),
            ("memories", memories.to_string()),
        ],
    )
}

pub fn weekly_prompt(facts: &str) -> String {
    render(WEEKLY_BRIEFING, &[("facts", facts.to_string())])
}

pub fn curriculum_prompt(facts: &str) -> String {
    render(CURRICULUM_NEXT, &[("facts", facts.to_string())])
}

pub fn match_prompt(facts: &str) -> String {
    render(MATCH_COACH, &[("facts", facts.to_string())])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_named_placeholders_only() {
        let rendered = render("a {{x}} b {{y}}", &[("x", "1".into())]);
        assert_eq!(rendered, "a 1 b {{y}}");
    }

    #[test]
    fn reflection_prompt_contains_anti_hallucination_rules() {
        let prompt = reflection_prompt("{}", "I moved too fast", "none");
        assert!(prompt.contains("Never evaluate a position yourself"));
        assert!(prompt.contains("NEXT_TIME_RULE"));
    }
}
