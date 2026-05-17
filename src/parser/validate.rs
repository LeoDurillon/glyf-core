//! Bracket balance validation for Glyf abbreviations.
//!
//! Checks that every `(`, `{`, and `[` in the input is matched by a
//! corresponding closer before the parser runs. A failed check short-circuits
//! the parse with [`crate::parser::GlyfError::UnmatchedBrackets`].

fn is_matching_opener(opener: &char, closer: &char) -> bool {
    match closer {
        ')' => &'(' == opener,
        ']' => &'[' == opener,
        '}' => &'{' == opener,
        _ => false,
    }
}

pub fn input_correctly_close(input: &str) -> bool {
    let mut opener_list: Vec<char> = Vec::new();

    for char in input.chars() {
        match char {
            '(' | '{' | '[' => {
                opener_list.push(char);
            }
            ')' | '}' | ']' => match opener_list.last() {
                Some(opener) => {
                    if !is_matching_opener(opener, &char) {
                        return false;
                    }
                    opener_list.pop();
                }
                None => {
                    return false;
                }
            },
            _ => {}
        }
    }

    opener_list.is_empty()
}

#[cfg(test)]
mod checker_test {
    use super::*;

    #[test]
    fn should_check_closing() {
        assert_eq!(true, input_correctly_close("html"));
        assert_eq!(true, input_correctly_close("html>p"));
        assert_eq!(true, input_correctly_close("(html>div>p)+icon"));
        assert_eq!(true, input_correctly_close("(html)+icon"));
        assert_eq!(true, input_correctly_close("(html>div>(p+div>p))+icon"));
        assert_eq!(
            true,
            input_correctly_close("(html.test.class>div:test:prop>(p+div>p))*3+icon>p")
        );
        assert_eq!(true, input_correctly_close("html>(div>p)*3"));
        assert_eq!(true, input_correctly_close("(div>p)*3"));
        assert_eq!(false, input_correctly_close("(div>p"));
        assert_eq!(false, input_correctly_close("div:foo={bar>p"));
        assert_eq!(false, input_correctly_close("div+("));
    }
}
