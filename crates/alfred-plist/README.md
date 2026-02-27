# nils-alfred-plist

Small utility crate for rendering Alfred workflow plist templates from tokenized text files.

## Public API Summary

- `render_template(template, vars)`: replaces `{{key}}` tokens with values from `BTreeMap<String, String>`.
- `render_template_file(template_path, output_path, vars)`: reads template file, renders tokens, creates parent
  directory when needed, and writes output.

## Documentation

- `docs/README.md`

## Validation

- `cargo check -p nils-alfred-plist`
- `cargo test -p nils-alfred-plist`
