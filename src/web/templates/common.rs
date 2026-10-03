#[derive(Debug)]
pub struct SoftwareTemplateData {
    pub version: &'static str,
    pub name: &'static str,
}

#[derive(Default, Debug)]
pub struct CommonTemplateData {
    pub software: SoftwareTemplateData,
}

impl Default for SoftwareTemplateData {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            name: env!("CARGO_PKG_NAME"),
        }
    }
}
