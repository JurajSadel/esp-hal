//! # PCNT - Channel Configuration
//!
//! ## Overview
//! The `channel` module allows users to configure and manage individual
//! channels of the `PCNT` peripheral. It provides methods to set various
//! parameters for each channel, such as control modes for signal edges, action
//! on control level, and configurations for positive and negative edge count
//! modes.

use core::marker::PhantomData;

pub use crate::pac::pcnt::unit::conf0::{CTRL_MODE as CtrlMode, EDGE_MODE as EdgeMode};
use crate::{
    gpio::{InputSignal, interconnect::PeripheralInput},
    peripherals::PCNT,
    system::GenericPeripheralGuard,
};

/// Represents a channel within a pulse counter unit.
pub struct Channel<'d, const UNIT: usize, const NUM: usize> {
    _phantom: PhantomData<&'d ()>,
    // Individual channels are not Send, since they share registers.
    _not_send: PhantomData<*const ()>,
    _guard: GenericPeripheralGuard<{ crate::system::Peripheral::Pcnt as u8 }>,
}

impl<const UNIT: usize, const NUM: usize> Channel<'_, UNIT, NUM> {
    /// return a new Channel
    pub(super) fn new() -> Self {
        let guard = GenericPeripheralGuard::new();

        Self {
            _phantom: PhantomData,
            _not_send: PhantomData,
            _guard: guard,
        }
    }

    /// Configures how the channel behaves based on the level of the control
    /// signal.
    ///
    /// * `low` - The behaviour of the channel when the control signal is low.
    /// * `high` - The behaviour of the channel when the control signal is high.
    pub fn set_ctrl_mode(&self, low: CtrlMode, high: CtrlMode) {
        let pcnt = PCNT::regs();
        let conf0 = pcnt.unit(UNIT).conf0();

        conf0.modify(|_, w| {
            w.ch_hctrl_mode(NUM as u8)
                .variant(high)
                .ch_lctrl_mode(NUM as u8)
                .variant(low)
        });
    }

    /// Configures how the channel affects the counter based on the transition
    /// made by the input signal.
    ///
    /// * `neg_edge` - The effect on the counter when the input signal goes 1 -> 0.
    /// * `pos_edge` - The effect on the counter when the input signal goes 0 -> 1.
    pub fn set_input_mode(&self, neg_edge: EdgeMode, pos_edge: EdgeMode) {
        let pcnt = PCNT::regs();
        let conf0 = pcnt.unit(UNIT).conf0();

        conf0.modify(|_, w| {
            w.ch_neg_mode(NUM as u8).variant(neg_edge);
            w.ch_pos_mode(NUM as u8).variant(pos_edge)
        });
    }

    /// Set the control signal (pin/high/low) for this channel
    pub fn set_ctrl_signal<'d>(&self, source: impl PeripheralInput<'d>) -> &Self {
        let signal = match UNIT {
            0 => match NUM {
                0 => InputSignal::PCNT0_CTRL_CH0,
                1 => InputSignal::PCNT0_CTRL_CH1,
                _ => unreachable!(),
            },
            1 => match NUM {
                0 => InputSignal::PCNT1_CTRL_CH0,
                1 => InputSignal::PCNT1_CTRL_CH1,
                _ => unreachable!(),
            },
            2 => match NUM {
                0 => InputSignal::PCNT2_CTRL_CH0,
                1 => InputSignal::PCNT2_CTRL_CH1,
                _ => unreachable!(),
            },
            3 => match NUM {
                0 => InputSignal::PCNT3_CTRL_CH0,
                1 => InputSignal::PCNT3_CTRL_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            4 => match NUM {
                0 => InputSignal::PCNT4_CTRL_CH0,
                1 => InputSignal::PCNT4_CTRL_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            5 => match NUM {
                0 => InputSignal::PCNT5_CTRL_CH0,
                1 => InputSignal::PCNT5_CTRL_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            6 => match NUM {
                0 => InputSignal::PCNT6_CTRL_CH0,
                1 => InputSignal::PCNT6_CTRL_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            7 => match NUM {
                0 => InputSignal::PCNT7_CTRL_CH0,
                1 => InputSignal::PCNT7_CTRL_CH1,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        if signal as usize <= property!("gpio.input_signal_max") {
            let source = source.into();
            source.set_input_enable(true);
            signal.connect_to(&source);
        }
        self
    }

    /// Set the edge signal (pin/high/low) for this channel
    pub fn set_edge_signal<'d>(&self, source: impl PeripheralInput<'d>) -> &Self {
        let signal = match UNIT {
            0 => match NUM {
                0 => InputSignal::PCNT0_SIG_CH0,
                1 => InputSignal::PCNT0_SIG_CH1,
                _ => unreachable!(),
            },
            1 => match NUM {
                0 => InputSignal::PCNT1_SIG_CH0,
                1 => InputSignal::PCNT1_SIG_CH1,
                _ => unreachable!(),
            },
            2 => match NUM {
                0 => InputSignal::PCNT2_SIG_CH0,
                1 => InputSignal::PCNT2_SIG_CH1,
                _ => unreachable!(),
            },
            3 => match NUM {
                0 => InputSignal::PCNT3_SIG_CH0,
                1 => InputSignal::PCNT3_SIG_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            4 => match NUM {
                0 => InputSignal::PCNT4_SIG_CH0,
                1 => InputSignal::PCNT4_SIG_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            5 => match NUM {
                0 => InputSignal::PCNT5_SIG_CH0,
                1 => InputSignal::PCNT5_SIG_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            6 => match NUM {
                0 => InputSignal::PCNT6_SIG_CH0,
                1 => InputSignal::PCNT6_SIG_CH1,
                _ => unreachable!(),
            },
            #[cfg(esp32)]
            7 => match NUM {
                0 => InputSignal::PCNT7_SIG_CH0,
                1 => InputSignal::PCNT7_SIG_CH1,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        if signal as usize <= property!("gpio.input_signal_max") {
            let source = source.into();
            source.set_input_enable(true);
            signal.connect_to(&source);
        }
        self
    }
}
