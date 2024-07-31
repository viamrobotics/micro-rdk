#[cfg(feature = "provisioning")]
pub mod server;

#[derive(Default, Clone)]
pub struct ProvisioningInfo(crate::proto::provisioning::v1::ProvisioningInfo);

impl ProvisioningInfo {
    pub fn set_fragment_id(&mut self, frag_id: String) {
        self.0.fragment_id = frag_id;
    }
    pub fn set_model(&mut self, model: String) {
        self.0.model = model;
    }
    pub fn set_manufacturer(&mut self, manufacturer: String) {
        self.0.manufacturer = manufacturer;
    }
    pub fn get_model(&self) -> &str {
        &self.0.model
    }
    pub fn get_manufacturer(&self) -> &str {
        &self.0.manufacturer
    }
}
