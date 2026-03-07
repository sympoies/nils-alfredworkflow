const DEFINE_PREFIX: &str = "def::";
const SUGGEST_PREFIX: &str = "sug::";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryToken {
    Empty,
    Smart { query: String },
    SuggestOnly { query: String },
    Define { entry: String },
    DefineMissingEntry,
    SuggestMissingQuery,
}

pub fn parse_query_token(raw_input: &str) -> QueryToken {
    let input = raw_input.trim();
    if input.is_empty() {
        return QueryToken::Empty;
    }

    if let Some(rest) = input.strip_prefix(DEFINE_PREFIX) {
        let entry = rest.trim();
        if entry.is_empty() {
            QueryToken::DefineMissingEntry
        } else {
            QueryToken::Define {
                entry: entry.to_string(),
            }
        }
    } else if let Some(rest) = input.strip_prefix(SUGGEST_PREFIX) {
        let query = rest.trim();
        if query.is_empty() {
            QueryToken::SuggestMissingQuery
        } else {
            QueryToken::SuggestOnly {
                query: query.to_string(),
            }
        }
    } else {
        QueryToken::Smart {
            query: input.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_parser_detects_empty_input() {
        assert_eq!(parse_query_token(" \t "), QueryToken::Empty);
    }

    #[test]
    fn token_parser_routes_plain_text_to_smart_mode() {
        assert_eq!(
            parse_query_token(" open "),
            QueryToken::Smart {
                query: "open".to_string(),
            }
        );
    }

    #[test]
    fn token_parser_routes_def_prefix_to_define_mode() {
        assert_eq!(
            parse_query_token("def::open"),
            QueryToken::Define {
                entry: "open".to_string(),
            }
        );
    }

    #[test]
    fn token_parser_trims_define_entry_value() {
        assert_eq!(
            parse_query_token("def::   open up  "),
            QueryToken::Define {
                entry: "open up".to_string(),
            }
        );
    }

    #[test]
    fn token_parser_flags_missing_define_entry() {
        assert_eq!(parse_query_token("def::  "), QueryToken::DefineMissingEntry);
    }

    #[test]
    fn token_parser_routes_suggest_prefix_to_suggest_only_mode() {
        assert_eq!(
            parse_query_token("sug::open"),
            QueryToken::SuggestOnly {
                query: "open".to_string(),
            }
        );
    }

    #[test]
    fn token_parser_trims_suggest_query_value() {
        assert_eq!(
            parse_query_token("sug::   open up  "),
            QueryToken::SuggestOnly {
                query: "open up".to_string(),
            }
        );
    }

    #[test]
    fn token_parser_flags_missing_suggest_query() {
        assert_eq!(
            parse_query_token("sug::  "),
            QueryToken::SuggestMissingQuery
        );
    }

    #[test]
    fn token_parser_is_case_sensitive_for_prefix() {
        assert_eq!(
            parse_query_token("DEF::open"),
            QueryToken::Smart {
                query: "DEF::open".to_string(),
            }
        );
    }
}
