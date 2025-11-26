pub trait StringExt {
    fn is_email(&self) -> bool;
}

impl StringExt for String {
    fn is_email(&self) -> bool {
        let has_at = self.contains('@');
        let has_dot = self.contains('.');
        let has_valid_length = self.len() >= 3;
        let parts: Vec<&str> = self.split('@').collect();
        let valid_parts = parts.len() == 2 && !parts[0].is_empty() && parts[0].len() <= 255 && !parts[1].is_empty();
        let domain_parts: Vec<&str> = parts.get(1).unwrap_or(&"").split('.').collect();
        let valid_domain = domain_parts.len() >= 2 && domain_parts.iter().all(|p| !p.is_empty() && p.len() <= 63);

        if !(has_at && has_dot && has_valid_length && valid_parts && valid_domain) {
            return false
        }

        true
    }
}