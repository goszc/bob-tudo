pub fn newer(remote: &str, current: &str) -> bool {
    fn parts(value: &str) -> Vec<u64> {
        value
            .trim()
            .trim_start_matches('v')
            .split(['.', '-'])
            .map(|part| {
                part.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }
    parts(remote) > parts(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("v0.1.1", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.1"));
    }
}
