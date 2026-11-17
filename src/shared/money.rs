// 19000000 kobo becomes "190,000.00", 0 becomes "0.00"
pub fn format_kobo_as_naira(kobo: i64) -> String {
    let naira = kobo / 100;
    let remainder = kobo % 100;

    let digits = naira.to_string();
    let mut with_commas = String::new();

    for (position, digit) in digits.chars().enumerate() {
        let digits_left = digits.len() - position;
        if position > 0 && digits_left % 3 == 0 {
            with_commas.push(',');
        }
        with_commas.push(digit);
    }

    format!("{}.{:02}", with_commas, remainder) // no semicolon
}

// "5,000.50" becomes 500050 kobo, no floats, nothing if the text is not a valid amount
pub fn parse_naira_to_kobo(text: &str) -> Option<i64> {
    // commas are just for reading
    let cleaned: String = text.trim().chars().filter(|c| *c != ',').collect();

    let (whole, fraction) = match cleaned.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (cleaned.as_str(), ""),
    };

    // nothing at all, or more than 2 decimal places
    if (whole.is_empty() && fraction.is_empty()) || fraction.len() > 2 {
        return None;
    }

    // digits only, so no minus sign and no letters
    let all_digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if !all_digits(whole) || !all_digits(fraction) {
        return None;
    }

    let whole_naira: i64 = if whole.is_empty() { 0 } else { whole.parse().ok()? };

    // "5" after the dot means 50 kobo, "05" means 5 kobo
    let kobo_part: i64 = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse().ok()?,
    };

    whole_naira.checked_mul(100)?.checked_add(kobo_part) // no semicolon
}
