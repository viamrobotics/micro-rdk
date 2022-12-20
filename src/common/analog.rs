#![allow(dead_code)]

pub struct FakeAnalogReader {
    name: String,
    value: u16,
}

impl FakeAnalogReader {
    pub fn new(name: String, value: u16) -> Self {
        Self { name, value }
    }
    fn internal_name(&self) -> String {
        self.name.clone()
    }
    fn internal_read(&self) -> anyhow::Result<u16> {
        Ok(self.value)
    }
}

impl AnalogReader<u16> for FakeAnalogReader {
    type Error = anyhow::Error;
    fn name(&self) -> String {
        self.internal_name()
    }
    fn read(&mut self) -> Result<u16, Self::Error> {
        self.internal_read()
    }
}

pub trait AnalogReader<Word> {
    type Error;
    fn read(&mut self) -> Result<Word, Self::Error>;
    fn name(&self) -> String;
}
