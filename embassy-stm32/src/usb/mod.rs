//! Universal Serial Bus (USB)

#[cfg_attr(usb, path = "usb.rs")]
#[cfg_attr(otg, path = "otg.rs")]
mod _version;
pub use _version::*;

use crate::interrupt::typelevel::Interrupt;
use crate::rcc;

/// clock, power initialization stuff that's common for USB and OTG.
fn common_init<T: Instance>() {
    // Check the USB clock is enabled and running at exactly 48 MHz.
    // frequency() will panic if not enabled
    let freq = T::frequency();

    // On the H7RS, the USBPHYC embeds a PLL accepting one of the input frequencies listed below and providing 48MHz to OTG_FS and 60MHz to OTG_HS internally
    #[cfg(any(stm32h7rs, all(stm32u5, peri_usb_otg_hs), all(stm32wba, peri_usb_otg_hs)))]
    if ![16_000_000, 19_200_000, 20_000_000, 24_000_000, 26_000_000, 32_000_000].contains(&freq.0) {
        panic!(
            "USB clock should be one of 16, 19.2, 20, 24, 26, 32Mhz but is {} Hz. Please double-check your RCC settings.",
            freq.0
        )
    }
    // Check frequency is within the 0.25% tolerance allowed by the spec.
    // Clock might not be exact 48Mhz due to rounding errors in PLL calculation, or if the user
    // has tight clock restrictions due to something else (like audio).
    #[cfg(not(any(stm32h7rs, all(stm32u5, peri_usb_otg_hs), all(stm32wba, peri_usb_otg_hs))))]
    if freq.0.abs_diff(48_000_000) > 120_000 {
        panic!(
            "USB clock should be 48Mhz but is {} Hz. Please double-check your RCC settings.",
            freq.0
        )
    }

    #[cfg(any(stm32l4, stm32l5, stm32wb, stm32u0))]
    critical_section::with(|_| crate::pac::PWR.cr2().modify(|w| w.set_usv(true)));

    #[cfg(pwr_h5)]
    critical_section::with(|_| crate::pac::PWR.usbscr().modify(|w| w.set_usb33sv(true)));

    #[cfg(stm32h7)]
    {
        // If true, VDD33USB is generated by internal regulator from VDD50USB
        // If false, VDD33USB and VDD50USB must be suplied directly with 3.3V (default on nucleo)
        // TODO: unhardcode
        let internal_regulator = false;

        // Enable USB power
        critical_section::with(|_| {
            crate::pac::PWR.cr3().modify(|w| {
                w.set_usb33den(true);
                w.set_usbregen(internal_regulator);
            })
        });

        // Wait for USB power to stabilize
        while !crate::pac::PWR.cr3().read().usb33rdy() {}
    }

    #[cfg(stm32h7rs)]
    {
        // If true, VDD33USB is generated by internal regulator from VDD50USB
        // If false, VDD33USB and VDD50USB must be suplied directly with 3.3V (default on nucleo)
        // TODO: unhardcode
        let internal_regulator = false;

        // Enable USB power
        critical_section::with(|_| {
            crate::pac::PWR.csr2().modify(|w| {
                w.set_usbregen(internal_regulator);
                w.set_usb33den(true);
                w.set_usbhsregen(true);
            })
        });

        // Wait for USB power to stabilize
        while !crate::pac::PWR.csr2().read().usb33rdy() {}
    }

    #[cfg(stm32u5)]
    {
        // Enable USB power
        critical_section::with(|_| {
            crate::pac::PWR.svmcr().modify(|w| {
                w.set_usv(true);
                w.set_uvmen(true);
            })
        });

        // Wait for USB power to stabilize
        while !crate::pac::PWR.svmsr().read().vddusbrdy() {}

        // Now set up transceiver power if it's a OTG-HS
        #[cfg(peri_usb_otg_hs)]
        {
            crate::pac::PWR.vosr().modify(|w| {
                w.set_usbpwren(true);
                w.set_usbboosten(true);
            });
            while !crate::pac::PWR.vosr().read().usbboostrdy() {}
        }
    }

    #[cfg(stm32wba)]
    {
        // Enable USB power
        critical_section::with(|_| {
            crate::pac::PWR.svmcr().modify(|w| {
                w.set_usv(crate::pac::pwr::vals::Usv::B_0X1);
            });
            crate::pac::PWR.vosr().modify(|w| {
                w.set_vdd11usbdis(false);
                w.set_usbpwren(true);
            });
        });

        // Wait for USB power to stabilize
        while !crate::pac::PWR.vosr().read().vdd11usbrdy() {}

        // Now set up transceiver power if it's a OTG-HS
        #[cfg(peri_usb_otg_hs)]
        {
            crate::pac::PWR.vosr().modify(|w| {
                w.set_usbboosten(true);
            });
            while !crate::pac::PWR.vosr().read().usbboostrdy() {}
        }
    }

    T::Interrupt::unpend();
    unsafe { T::Interrupt::enable() };

    rcc::enable_and_reset::<T>();
}
