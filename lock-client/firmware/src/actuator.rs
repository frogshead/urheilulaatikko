use esp_hal::gpio::Output;
use lock_client_core::lock::Actuator;

/// Drives the GPIO pin feeding the level-shifter/relay module that controls
/// the Asia-Teco electric drop bolt (fail-secure/NC — it stays locked with no
/// signal, and releases only while this pin is driven high).
pub struct BoltActuator<'d> {
    relay: Output<'d>,
}

impl<'d> BoltActuator<'d> {
    pub fn new(relay: Output<'d>) -> Self {
        BoltActuator { relay }
    }
}

impl Actuator for BoltActuator<'_> {
    fn unlock(&mut self) {
        self.relay.set_high();
    }

    fn lock(&mut self) {
        self.relay.set_low();
    }
}
