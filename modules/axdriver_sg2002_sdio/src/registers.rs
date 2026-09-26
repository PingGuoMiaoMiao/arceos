const MAX_DIVISOR: u32 = 2046;
const DIVIDER_MASK: u16 = 0x00ff;
const DIVIDER_HIGH_MASK: u16 = 0x0300;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockDivider {
    pub register_bits: u16,
    pub actual_hz: u32,
}

/// Calculates the SDHCI v3 divided-clock encoding used by CV181x.
pub const fn clock_divider(base_hz: u32, requested_hz: u32) -> Option<ClockDivider> {
    if base_hz == 0 || requested_hz == 0 {
        return None;
    }

    let real_divisor = if base_hz <= requested_hz {
        1
    } else {
        let mut divisor = 2;
        while divisor <= MAX_DIVISOR && base_hz / divisor > requested_hz {
            divisor += 2;
        }
        if divisor > MAX_DIVISOR {
            return None;
        }
        divisor
    };

    let encoded_divisor = (real_divisor >> 1) as u16;
    let register_bits = ((encoded_divisor & DIVIDER_MASK) << 8)
        | (((encoded_divisor & DIVIDER_HIGH_MASK) >> 8) << 6);

    Some(ClockDivider {
        register_bits,
        actual_hz: base_hz / real_divisor,
    })
}
