/// Validation provider
pub trait Validator {
    fn is_valid(&self, line: &str) -> bool;
}

impl Validator for () {
    fn is_valid(&self, _line: &str) -> bool {
        true
    }
}
