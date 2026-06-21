// `randomer-cli` production paths now route every fallible call through
// `?` plus a `NILS_RANDOMER_<NNN>` code (see
// `docs/specs/cli-error-code-registry.md`); lock the gate so future
// regressions surface as build errors instead of silent panics. Tests
// keep `unwrap()` / `expect()` for compactness — see the `#[allow]` on
// `mod tests` below.
#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::fmt;

use alfred_core::{Feedback, Item, ItemIcon, ItemModifier};
use rand::{Rng, RngExt};
use uuid::Uuid;

const UNIT_LETTER_VALUES: [u32; 26] = [
    10, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36,
    37, 38,
];

/// Upper bound on a single `generate` request. Each value allocates an Alfred
/// `Item`, and an Alfred Script Filter never renders thousands of rows, so cap
/// the request to keep memory bounded and reject obviously hostile inputs.
const MAX_COUNT: usize = 1000;

const ALL_FORMATS: [Format; 11] = [
    Format::Email,
    Format::Imei,
    Format::Unit,
    Format::Uuid,
    Format::Int,
    Format::Decimal,
    Format::Percent,
    Format::Currency,
    Format::Hex,
    Format::Otp,
    Format::Phone,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Email,
    Imei,
    Unit,
    Uuid,
    Int,
    Decimal,
    Percent,
    Currency,
    Hex,
    Otp,
    Phone,
}

impl Format {
    pub fn all() -> &'static [Format] {
        &ALL_FORMATS
    }

    pub fn parse(input: &str) -> Option<Self> {
        let normalized = input.trim().to_ascii_lowercase();
        Self::all()
            .iter()
            .copied()
            .find(|format| format.key() == normalized)
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Imei => "imei",
            Self::Unit => "unit",
            Self::Uuid => "uuid",
            Self::Int => "int",
            Self::Decimal => "decimal",
            Self::Percent => "percent",
            Self::Currency => "currency",
            Self::Hex => "hex",
            Self::Otp => "otp",
            Self::Phone => "phone",
        }
    }

    fn icon_path(self) -> String {
        format!("assets/icons/{}.png", self.key())
    }

    fn generate_with_rng<R: Rng + ?Sized>(self, rng: &mut R) -> String {
        match self {
            Self::Email => random_email(rng),
            Self::Imei => random_imei(rng),
            Self::Unit => random_unit_number(rng),
            Self::Uuid => random_uuid(),
            Self::Int => random_int(rng),
            Self::Decimal => random_decimal(rng),
            Self::Percent => random_percent(rng),
            Self::Currency => random_currency(rng),
            Self::Hex => random_hex(rng),
            Self::Otp => random_otp(rng),
            Self::Phone => random_phone(rng),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RandomerError {
    UnknownFormat(String),
    InvalidCount(usize),
    CountTooLarge(usize),
}

impl fmt::Display for RandomerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormat(format) => write!(f, "unknown format: {format}"),
            Self::InvalidCount(count) => write!(f, "count must be at least 1 (got {count})"),
            Self::CountTooLarge(count) => {
                write!(f, "count must be at most {MAX_COUNT} (got {count})")
            }
        }
    }
}

impl std::error::Error for RandomerError {}

pub fn filter_formats(query: Option<&str>) -> Vec<Format> {
    let query = query.unwrap_or_default().trim().to_ascii_lowercase();
    Format::all()
        .iter()
        .copied()
        .filter(|format| query.is_empty() || format.key().contains(&query))
        .collect()
}

pub fn list_formats_feedback(query: Option<&str>) -> Feedback {
    let mut rng = rand::rng();
    list_formats_feedback_with_rng(query, &mut rng)
}

pub fn list_types_feedback(query: Option<&str>) -> Feedback {
    let mut rng = rand::rng();
    list_types_feedback_with_rng(query, &mut rng)
}

pub fn generate_feedback(format_name: &str, count: usize) -> Result<Feedback, RandomerError> {
    let mut rng = rand::rng();
    generate_feedback_with_rng(format_name, count, &mut rng)
}

fn list_formats_feedback_with_rng<R: Rng + ?Sized>(query: Option<&str>, rng: &mut R) -> Feedback {
    let items = filter_formats(query)
        .into_iter()
        .map(|format| {
            let sample = format.generate_with_rng(rng);
            Item::new(sample.clone())
                .with_subtitle(format!(
                    "{} · Enter: copy sample · Cmd+Enter: show 10 values",
                    format.key()
                ))
                .with_arg(sample)
                .with_valid(true)
                .with_icon(ItemIcon::new(format.icon_path()))
                .with_mod(
                    "cmd",
                    ItemModifier::new()
                        .with_arg(format.key())
                        .with_subtitle(format!("show 10 values for {}", format.key()))
                        .with_variable("RANDOMER_FORMAT", format.key()),
                )
        })
        .collect();

    Feedback::new(items)
}

fn list_types_feedback_with_rng<R: Rng + ?Sized>(query: Option<&str>, rng: &mut R) -> Feedback {
    let items = filter_formats(query)
        .into_iter()
        .map(|format| {
            let sample = format.generate_with_rng(rng);
            Item::new(format.key())
                .with_subtitle(format!("sample: {sample} · Enter: show 10 values"))
                .with_arg(format.key())
                .with_valid(true)
                .with_icon(ItemIcon::new(format.icon_path()))
                .with_variable("RANDOMER_FORMAT", format.key())
        })
        .collect();

    Feedback::new(items)
}

fn generate_feedback_with_rng<R: Rng + ?Sized>(
    format_name: &str,
    count: usize,
    rng: &mut R,
) -> Result<Feedback, RandomerError> {
    if count == 0 {
        return Err(RandomerError::InvalidCount(count));
    }
    if count > MAX_COUNT {
        return Err(RandomerError::CountTooLarge(count));
    }

    let format = Format::parse(format_name)
        .ok_or_else(|| RandomerError::UnknownFormat(format_name.trim().to_ascii_lowercase()))?;

    let items = (0..count)
        .map(|_| {
            let value = format.generate_with_rng(rng);
            Item::new(value.clone())
                .with_subtitle(format.key())
                .with_arg(value)
                .with_valid(true)
                .with_icon(ItemIcon::new(format.icon_path()))
        })
        .collect();

    Ok(Feedback::new(items))
}

fn random_alpha_string<R: Rng + ?Sized>(rng: &mut R, size: usize) -> String {
    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    (0..size)
        .map(|_| {
            let index = rng.random_range(0..LETTERS.len());
            char::from(LETTERS[index])
        })
        .collect()
}

fn random_email<R: Rng + ?Sized>(rng: &mut R) -> String {
    format!(
        "{}@{}.com",
        random_alpha_string(rng, 10),
        random_alpha_string(rng, 7)
    )
    .to_ascii_lowercase()
}

fn random_imei<R: Rng + ?Sized>(rng: &mut R) -> String {
    let mut digits: Vec<u32> = (0..14)
        .map(|_| u32::from(rng.random_range(0..=9u8)))
        .collect();
    let checksum = imei_checksum_for_body(&digits);
    digits.push(checksum);

    // Every value comes from `rng.random_range(0..=9u8)`, so the cast is
    // already in-bounds — no panic surface even with `unwrap_used` denied.
    digits
        .into_iter()
        .map(|digit| char::from(b'0' + (digit as u8)))
        .collect()
}

fn imei_checksum_for_body(body: &[u32]) -> u32 {
    let transformed_sum: u32 = body
        .iter()
        .enumerate()
        .map(|(index, digit)| {
            let doubled = if index % 2 == 1 { digit * 2 } else { *digit };
            doubled / 10 + doubled % 10
        })
        .sum();

    (transformed_sum * 9) % 10
}

fn random_uppercase_letter<R: Rng + ?Sized>(rng: &mut R) -> char {
    char::from(b'A' + rng.random_range(0..26u8))
}

fn random_unit_number<R: Rng + ?Sized>(rng: &mut R) -> String {
    const FOURTH_LETTERS: [char; 3] = ['U', 'J', 'Z'];

    loop {
        let mut candidate = String::with_capacity(11);
        for _ in 0..3 {
            candidate.push(random_uppercase_letter(rng));
        }
        candidate.push(FOURTH_LETTERS[rng.random_range(0..FOURTH_LETTERS.len())]);
        for _ in 0..6 {
            candidate.push(char::from(b'0' + rng.random_range(0..=9u8)));
        }

        let checksum = unit_checksum(candidate.as_str());
        if checksum <= 9 {
            // Guarded by `checksum <= 9`, so the truncating cast is safe.
            candidate.push(char::from(b'0' + (checksum as u8)));
            return candidate;
        }
    }
}

fn unit_checksum(base: &str) -> u32 {
    let values = base.chars().map(unit_value);
    values
        .enumerate()
        .map(|(index, value)| value * (1u32 << index))
        .sum::<u32>()
        % 11
}

fn unit_value(ch: char) -> u32 {
    if ch.is_ascii_digit() {
        // `is_ascii_digit()` guarantees `(ch as u32) - (b'0' as u32)` is
        // in `0..=9`, so we skip the panicky `to_digit` path entirely.
        u32::from(ch as u8 - b'0')
    } else {
        unit_letter_value(ch)
    }
}

fn unit_letter_value(ch: char) -> u32 {
    let letter = ch.to_ascii_uppercase();
    debug_assert!(letter.is_ascii_uppercase());
    let index = usize::from((letter as u8) - b'A');
    UNIT_LETTER_VALUES[index]
}

fn random_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn random_int<R: Rng + ?Sized>(rng: &mut R) -> String {
    rng.random_range(0u64..=9_999_999_999u64).to_string()
}

fn random_decimal<R: Rng + ?Sized>(rng: &mut R) -> String {
    let whole = rng.random_range(0u64..=999_999u64);
    let fraction = rng.random_range(0u8..=99u8);
    format!("{whole}.{fraction:02}")
}

fn random_percent<R: Rng + ?Sized>(rng: &mut R) -> String {
    let basis_points = rng.random_range(0u32..=10_000u32);
    format!("{}.{:02}%", basis_points / 100, basis_points % 100)
}

fn random_currency<R: Rng + ?Sized>(rng: &mut R) -> String {
    let dollars = rng.random_range(0u64..=99_999_999u64);
    let cents = rng.random_range(0u8..=99u8);
    format!("${}.{:02}", with_thousands_separators(dollars), cents)
}

fn with_thousands_separators(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped_rev = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped_rev.push(',');
        }
        grouped_rev.push(ch);
    }
    grouped_rev.chars().rev().collect()
}

fn random_hex<R: Rng + ?Sized>(rng: &mut R) -> String {
    let value = rng.random_range(0u32..=u32::MAX);
    format!("0x{value:08X}")
}

fn random_otp<R: Rng + ?Sized>(rng: &mut R) -> String {
    let value = rng.random_range(0u32..=999_999u32);
    format!("{value:06}")
}

fn random_phone<R: Rng + ?Sized>(rng: &mut R) -> String {
    let suffix: String = (0..8)
        .map(|_| char::from(b'0' + rng.random_range(0..=9u8)))
        .collect();
    format!("09{suffix}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn supported_formats_match_contract_order() {
        let keys: Vec<_> = Format::all().iter().map(|format| format.key()).collect();
        assert_eq!(
            keys,
            vec![
                "email", "imei", "unit", "uuid", "int", "decimal", "percent", "currency", "hex",
                "otp", "phone"
            ]
        );
    }

    #[test]
    fn format_parse_is_case_insensitive_and_trimmed() {
        assert_eq!(Format::parse(" Email "), Some(Format::Email));
        assert_eq!(Format::parse("IMEI"), Some(Format::Imei));
        assert_eq!(Format::parse("UnIt"), Some(Format::Unit));
        assert_eq!(Format::parse("otp"), Some(Format::Otp));
        assert_eq!(Format::parse(" Phone "), Some(Format::Phone));
        assert_eq!(Format::parse("unknown"), None);
    }

    #[test]
    fn query_filter_is_case_insensitive_contains() {
        let all = filter_formats(None);
        assert_eq!(all, Format::all().to_vec());

        let filtered = filter_formats(Some("IMEI"));
        assert_eq!(filtered, vec![Format::Imei]);

        let none = filter_formats(Some("not-found"));
        assert!(none.is_empty());
    }

    #[test]
    fn list_formats_feedback_contains_menu_contract_fields() {
        let mut rng = seeded_rng();
        let feedback = list_formats_feedback_with_rng(Some("hex"), &mut rng);

        assert_eq!(feedback.items.len(), 1);
        let item = &feedback.items[0];
        assert_eq!(item.arg.as_deref(), Some(item.title.as_str()));

        let subtitle = item.subtitle.as_deref().expect("subtitle should be set");
        assert!(subtitle.contains("hex"));
        assert!(subtitle.contains("Enter"));
        assert!(subtitle.contains("Cmd+Enter"));

        assert_eq!(
            item.icon.as_ref().map(|icon| icon.path.as_str()),
            Some("assets/icons/hex.png")
        );

        let cmd_mod = item
            .mods
            .as_ref()
            .and_then(|mods| mods.get("cmd"))
            .expect("cmd modifier should be present");
        assert_eq!(cmd_mod.arg.as_deref(), Some("hex"));
        assert_eq!(
            cmd_mod
                .variables
                .as_ref()
                .and_then(|vars| vars.get("RANDOMER_FORMAT"))
                .map(String::as_str),
            Some("hex")
        );
        assert!(
            cmd_mod
                .subtitle
                .as_deref()
                .is_some_and(|subtitle| subtitle.contains("10"))
        );
    }

    #[test]
    fn generate_feedback_emits_requested_count_and_fields() {
        let mut rng = seeded_rng();
        let feedback = generate_feedback_with_rng("OtP", 3, &mut rng).expect("should generate");

        assert_eq!(feedback.items.len(), 3);
        for item in feedback.items {
            assert_eq!(item.arg.as_deref(), Some(item.title.as_str()));
            assert_eq!(item.subtitle.as_deref(), Some("otp"));
            assert_eq!(
                item.icon.as_ref().map(|icon| icon.path.as_str()),
                Some("assets/icons/otp.png")
            );
        }
    }

    #[test]
    fn generate_feedback_rejects_unknown_format() {
        let mut rng = seeded_rng();
        let err =
            generate_feedback_with_rng("unknown", 1, &mut rng).expect_err("should reject format");
        assert_eq!(err, RandomerError::UnknownFormat(String::from("unknown")));
    }

    #[test]
    fn generate_feedback_rejects_zero_count() {
        let mut rng = seeded_rng();
        let err = generate_feedback_with_rng("email", 0, &mut rng).expect_err("should reject 0");
        assert_eq!(err, RandomerError::InvalidCount(0));
    }

    #[test]
    fn generate_feedback_rejects_count_above_max() {
        let mut rng = seeded_rng();
        let err = generate_feedback_with_rng("email", MAX_COUNT + 1, &mut rng)
            .expect_err("should reject count above MAX_COUNT");
        assert_eq!(err, RandomerError::CountTooLarge(MAX_COUNT + 1));
    }

    #[test]
    fn generate_feedback_accepts_count_at_max() {
        let mut rng = seeded_rng();
        let feedback = generate_feedback_with_rng("otp", MAX_COUNT, &mut rng)
            .expect("MAX_COUNT itself should be accepted");
        assert_eq!(feedback.items.len(), MAX_COUNT);
    }

    #[test]
    fn list_types_feedback_contains_type_selector_contract_fields() {
        let mut rng = seeded_rng();
        let feedback = list_types_feedback_with_rng(Some("in"), &mut rng);

        assert_eq!(feedback.items.len(), 1);
        let item = &feedback.items[0];
        assert_eq!(item.title, "int");
        assert_eq!(item.arg.as_deref(), Some("int"));
        assert_eq!(
            item.icon.as_ref().map(|icon| icon.path.as_str()),
            Some("assets/icons/int.png")
        );
        assert!(
            item.subtitle
                .as_deref()
                .is_some_and(|subtitle| subtitle.contains("show 10 values"))
        );
        assert_eq!(
            item.variables
                .as_ref()
                .and_then(|vars| vars.get("RANDOMER_FORMAT"))
                .map(String::as_str),
            Some("int")
        );
    }

    #[test]
    fn format_email_shape_matches_legacy_contract() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Email.generate_with_rng(&mut rng);
            let (local, domain_tld) = value.split_once('@').expect("email should contain @");
            let domain = domain_tld
                .strip_suffix(".com")
                .expect("email should end with .com");

            assert_eq!(local.len(), 10);
            assert_eq!(domain.len(), 7);
            assert!(local.chars().all(|ch| ch.is_ascii_lowercase()));
            assert!(domain.chars().all(|ch| ch.is_ascii_lowercase()));
        }
    }

    #[test]
    fn format_imei_has_15_digits_and_valid_checksum() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Imei.generate_with_rng(&mut rng);
            assert_eq!(value.len(), 15);
            assert!(value.chars().all(|ch| ch.is_ascii_digit()));
            assert!(imei_has_valid_checksum(value.as_str()));
        }
    }

    #[test]
    fn format_unit_has_legacy_shape_and_checksum() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Unit.generate_with_rng(&mut rng);
            assert_eq!(value.len(), 11);
            assert!(value.chars().take(3).all(|ch| ch.is_ascii_uppercase()));
            assert!(matches!(value.chars().nth(3), Some('U' | 'J' | 'Z')));
            assert!(value.chars().skip(4).take(6).all(|ch| ch.is_ascii_digit()));
            assert!(
                value
                    .chars()
                    .last()
                    .is_some_and(|checksum| checksum.is_ascii_digit())
            );
            assert!(unit_has_valid_checksum(value.as_str()));
        }
    }

    #[test]
    fn format_uuid_is_rfc4122_v4() {
        for _ in 0..50 {
            let value = Format::Uuid.generate_with_rng(&mut seeded_rng());
            let parsed = Uuid::parse_str(value.as_str()).expect("uuid should parse");
            assert_eq!(parsed.get_version_num(), 4);
        }
    }

    #[test]
    fn format_int_is_digits_only() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Int.generate_with_rng(&mut rng);
            assert!(!value.is_empty());
            assert!(value.chars().all(|ch| ch.is_ascii_digit()));
        }
    }

    #[test]
    fn format_decimal_has_fixed_two_decimals() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Decimal.generate_with_rng(&mut rng);
            let (whole, fraction) = value
                .split_once('.')
                .expect("decimal should contain decimal point");
            assert!(whole.chars().all(|ch| ch.is_ascii_digit()));
            assert_eq!(fraction.len(), 2);
            assert!(fraction.chars().all(|ch| ch.is_ascii_digit()));
        }
    }

    #[test]
    fn format_percent_has_suffix_and_bounds() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Percent.generate_with_rng(&mut rng);
            let number = value
                .strip_suffix('%')
                .expect("percent should end with % suffix");
            let (whole, fraction) = number
                .split_once('.')
                .expect("percent should have decimals");
            assert!(whole.chars().all(|ch| ch.is_ascii_digit()));
            assert_eq!(fraction.len(), 2);
            assert!(fraction.chars().all(|ch| ch.is_ascii_digit()));

            let whole_number: u32 = whole.parse().expect("whole should parse");
            assert!(whole_number <= 100);
            if whole_number == 100 {
                assert_eq!(fraction, "00");
            }
        }
    }

    #[test]
    fn format_currency_has_symbol_grouping_and_scale() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Currency.generate_with_rng(&mut rng);
            assert!(is_currency_shape(value.as_str()));
        }
    }

    #[test]
    fn format_hex_has_prefix_fixed_width_and_uppercase() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Hex.generate_with_rng(&mut rng);
            assert_eq!(value.len(), 10);
            assert!(value.starts_with("0x"));
            assert!(
                value
                    .chars()
                    .skip(2)
                    .all(|ch| ch.is_ascii_digit() || ('A'..='F').contains(&ch))
            );
        }
    }

    #[test]
    fn format_otp_is_six_digits_zero_padded() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Otp.generate_with_rng(&mut rng);
            assert_eq!(value.len(), 6);
            assert!(value.chars().all(|ch| ch.is_ascii_digit()));
        }
    }

    #[test]
    fn format_phone_is_taiwan_mobile_shape() {
        let mut rng = seeded_rng();
        for _ in 0..100 {
            let value = Format::Phone.generate_with_rng(&mut rng);
            assert_eq!(value.len(), 10);
            assert!(value.starts_with("09"));
            assert!(value.chars().all(|ch| ch.is_ascii_digit()));
        }
    }

    fn imei_has_valid_checksum(value: &str) -> bool {
        if value.len() != 15 || !value.chars().all(|ch| ch.is_ascii_digit()) {
            return false;
        }

        let digits: Vec<u32> = value
            .chars()
            .map(|ch| ch.to_digit(10).expect("digit"))
            .collect();
        imei_checksum_for_body(&digits[..14]) == digits[14]
    }

    fn unit_has_valid_checksum(value: &str) -> bool {
        if value.len() != 11 {
            return false;
        }

        let (base, checksum_str) = value.split_at(10);
        let checksum_digit = match checksum_str.chars().next().and_then(|ch| ch.to_digit(10)) {
            Some(digit) => digit,
            None => return false,
        };
        unit_checksum(base) == checksum_digit
    }

    fn is_currency_shape(value: &str) -> bool {
        if !value.starts_with('$') {
            return false;
        }

        let (whole, fraction) = match value[1..].split_once('.') {
            Some(parts) => parts,
            None => return false,
        };
        if fraction.len() != 2 || !fraction.chars().all(|ch| ch.is_ascii_digit()) {
            return false;
        }

        let groups: Vec<&str> = whole.split(',').collect();
        if groups.is_empty() {
            return false;
        }
        if !(1..=3).contains(&groups[0].len()) || !groups[0].chars().all(|ch| ch.is_ascii_digit()) {
            return false;
        }
        groups
            .iter()
            .skip(1)
            .all(|group| group.len() == 3 && group.chars().all(|ch| ch.is_ascii_digit()))
    }
}
