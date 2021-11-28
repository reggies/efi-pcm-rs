use core::fmt;

pub struct VolumeKnobCapabilities(u32);

impl VolumeKnobCapabilities {
    pub fn from(caps: u32) -> VolumeKnobCapabilities {
        VolumeKnobCapabilities(caps)
    }
    pub fn num_steps(&self) -> u32 {
        self.0 & 0x7f
    }
    pub fn delta(&self) -> bool {
        (self.0 >> 7) & 1 == 1
    }
}

impl fmt::Debug for VolumeKnobCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VolumeKnobCapabilities")
            .field("num_steps", &self.num_steps())
            .field("delta", &self.delta())
            .finish()
    }
}

pub struct AmpGain(u32);

impl AmpGain {
    pub fn from(gain: u32) -> AmpGain {
        AmpGain(gain)
    }
    pub fn gain(&self) -> u32 {
        self.0 & 0x7f
    }
    pub fn mute(&self) -> bool {
        (self.0 >> 7) & 1 == 1
    }
}

impl fmt::Debug for AmpGain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AmpGain")
            .field("gain", &self.gain())
            .field("mute", &self.mute())
            .finish()
    }
}

pub struct AmpCapabilities(u32);

impl fmt::Debug for AmpCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AmpCapabilities")
            .field("offset", &self.offset())
            .field("num_steps", &self.num_steps())
            .field("step_size", &self.step_size())
            .field("mute", &self.mute())
            .finish()
    }
}

impl AmpCapabilities {
    pub fn from(caps: u32) -> AmpCapabilities {
        AmpCapabilities(caps)
    }
    pub fn offset(&self) -> u32 {
        self.0 & 0x7f
    }
    pub fn num_steps(&self) -> u32 {
        (self.0 >> 8) & 0x7f
    }
    pub fn step_size(&self) -> u32 {
        (self.0 >> 16) & 0x7f
    }
    pub fn mute(&self) -> bool {
        (self.0 >> 31) & 1 == 1
    }
}

pub struct PinCapabilities(u32);

impl fmt::Debug for PinCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PinCapabilities")
            .field("impedance_sense_capable", &self.impedance_sense_capable())
            .field("trigger_required", &self.trigger_required())
            .field("presence_detect_capable", &self.presence_detect_capable())
            .field("headphone_drive_capable", &self.headphone_drive_capable())
            .field("output_capable", &self.output_capable())
            .field("input_capable", &self.input_capable())
            .field("balanced_io_pins", &self.balanced_io_pins())
            .field("hdmi", &self.hdmi())
            .field("vref_control", &self.vref_control())
            .field("eapd_capable", &self.eapd_capable())
            .field("display_port", &self.display_port())
            .field("high_bit_rate", &self.high_bit_rate())
            .finish()
    }
}

impl PinCapabilities {
    pub fn from(caps: u32) -> PinCapabilities {
        PinCapabilities(caps)
    }
    pub fn impedance_sense_capable(&self) -> u32 {
        self.0 & 1
    }
    pub fn trigger_required(&self) -> u32 {
        (self.0 >> 1) & 1
    }
    pub fn presence_detect_capable(&self) -> u32 {
        (self.0 >> 2) & 1
    }
    pub fn headphone_drive_capable(&self) -> u32 {
        (self.0 >> 3) & 1
    }
    pub fn output_capable(&self) -> u32 {
        (self.0 >> 4) & 1
    }
    pub fn input_capable(&self) -> u32 {
        (self.0 >> 5) & 1
    }
    pub fn balanced_io_pins(&self) -> bool {
        (self.0 >> 6) & 1 == 1
    }
    pub fn hdmi(&self) -> u32 {
        (self.0 >> 7) & 1
    }
    pub fn vref_control(&self) -> u32 {
        (self.0 >> 8) & 0b11111111
    }
    pub fn eapd_capable(&self) -> bool {
        (self.0 >> 16) & 1 == 1
    }
    pub fn display_port(&self) -> u32 {
        (self.0 >> 24) & 1
    }
    pub fn high_bit_rate(&self) -> u32 {
        (self.0 >> 27) & 1
    }
}

pub struct PinConfig(u32);

impl fmt::Debug for PinConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PinConfig")
            .field("sequence", &self.sequence())
            .field("association", &self.association())
            .field("misc", &self.misc())
            .field("color", &self.color())
            .field("typ", &self.typ())
            .field("device", &self.device())
            .field("location", &self.location())
            .field("port_connectivity", &self.port_connectivity())
            .finish()
    }
}

impl PinConfig {
    pub fn from(cfg: u32) -> PinConfig {
        PinConfig(cfg)
    }
    pub fn sequence(&self) -> u32 {
        (self.0 >> 0) & 0b1111
    }
    pub fn association(&self) -> u32 {
        (self.0 >> 4) & 0b1111
    }
    pub fn misc(&self) -> u32 {
        (self.0 >> 8) & 0xf
    }
    pub fn color(&self) -> u32 {
        (self.0 >> 12) & 0b1111
    }
    pub fn typ(&self) -> u32 {
        (self.0 >> 16) & 0b1111
    }
    pub fn device(&self) -> u32 {
        (self.0 >> 20) & 0b1111
    }
    pub fn location(&self) -> u32 {
        (self.0 >> 24) & 0b111111
    }
    pub fn port_connectivity(&self) -> u32 {
        (self.0 >> 30) & 0b11
    }
}

pub struct WidgetCapabilities(u32);

impl fmt::Debug for WidgetCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetCapabilities")
            .field("channels", &self.channels())
            .field("typ", &self.typ())
            .field("stereo", &self.stereo())
            .field("in_amp_present", &self.in_amp_present())
            .field("out_amp_present", &self.out_amp_present())
            .field("amp_param_override", &self.amp_param_override())
            .field("format_override", &self.format_override())
            .field("stripe", &self.stripe())
            .field("proc_widget", &self.proc_widget())
            .field("unsol_capable", &self.unsol_capable())
            .field("connection_list", &self.connection_list())
            .field("digital", &self.digital())
            .field("power_ctl", &self.power_ctl())
            .field("lr_swap", &self.lr_swap())
            .field("cp_caps", &self.cp_caps())
            .field("delay", &self.delay())
            .finish()
    }
}

impl WidgetCapabilities {
    pub fn from(caps: u32) -> WidgetCapabilities {
        WidgetCapabilities (caps)
    }
    pub fn channels(&self) -> u32 {
        2 * (((self.0 >> 13) & 0x7) + 1)
    }
    pub fn typ(&self) -> u32 {
        (self.0 >> 20) & 0xf
    }
    pub fn stereo(&self) -> u32 {
        self.0 & 0x1
    }
    pub fn in_amp_present(&self) -> u32 {
        (self.0 >> 1) & 0x1
    }
    pub fn out_amp_present(&self) -> u32 {
        (self.0 >> 2) & 0x1
    }
    pub fn amp_param_override(&self) -> u32 {
        (self.0 >> 3) & 0x1
    }
    pub fn format_override(&self) -> u32 {
        (self.0 >> 4) & 0x1
    }
    pub fn stripe(&self) -> u32 {
        (self.0 >> 5) & 0x1
    }
    pub fn proc_widget(&self) -> u32 {
        (self.0 >> 6) & 0x1
    }
    pub fn unsol_capable(&self) -> u32 {
        (self.0 >> 7) & 0x1
    }
    pub fn connection_list(&self) -> u32 {
        (self.0 >> 8) & 0x1
    }
    pub fn digital(&self) -> u32 {
        (self.0 >> 9) & 0x1
    }
    pub fn power_ctl(&self) -> u32 {
        (self.0 >> 10) & 0x1
    }
    pub fn lr_swap(&self) -> u32 {
        (self.0 >> 11) & 0x1
    }
    pub fn cp_caps(&self) -> u32 {
        (self.0 >> 12) & 0x1
    }
    pub fn delay(&self) -> u32 {
        (self.0 >> 16) & 0xf
    }
}

pub struct GlobalCapabilities(u16);

impl fmt::Debug for GlobalCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlobalCapabilities")
            .field("out_streams", &self.out_streams())
            .field("in_streams", &self.in_streams())
            .field("bd_streams", &self.bd_streams())
            .field("sdo_signals", &self.sdo_signals())
            .field("ok_64", &self.ok_64())
            .finish()
    }
}

impl GlobalCapabilities {
    pub fn from(gcap: u16) -> GlobalCapabilities {
        GlobalCapabilities(gcap)
    }
    pub fn out_streams(&self) -> u16 {
        (self.0 >> 12) & 0b1111
    }
    pub fn in_streams(&self) -> u16 {
        (self.0 >> 8) & 0b1111
    }
    pub fn bd_streams(&self) -> u16 {
        (self.0 >> 3) & 0b11111
    }
    pub fn sdo_signals(&self) -> u16 {
        (self.0 >> 1) & 0b11
    }
    pub fn ok_64(&self) -> bool {
        (self.0 & 0x1) == 0x1
    }
}
