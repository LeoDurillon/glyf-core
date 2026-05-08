use std::collections::HashMap;

pub fn input_correctly_close(input: &str) -> bool {
    let mut opener: Vec<char> = Vec::new();

    let closer_opener_map = HashMap::from([(')', '('), (']', '['), ('}', '{')]);

    for char in input.chars() {
        match char {
            '(' | '{' | '[' => {
                opener.push(char);
            }
            ')' | '}' | ']' => match opener.last() {
                Some(v) => {
                    if Some(v) != closer_opener_map.get(&char) {
                        break;
                    }
                    opener.pop();
                }
                None => {
                    return false;
                }
            },
            _ => {}
        }
    }

    return opener.len() == 0;
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
